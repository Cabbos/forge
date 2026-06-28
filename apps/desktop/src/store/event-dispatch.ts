import type { SessionState, StreamEvent } from "../lib/protocol";
import { queryClient } from "../lib/query-client";
import { getModelContextWindow } from "../lib/providers";
import {
  applyConfirmResponseToBlocks,
  applyCompactResultToBlocks,
  applyFileIoToBlocks,
  applyShellStartToBlocks,
  closeInterruptedConfirmBlocks,
  eventToBlock,
  findShellTargetBlockIndex,
  findToolResultTargetBlockIndex,
  interruptedToolResultMetadata,
  isSameAsLastDeliveryBlock,
} from "./blocks";
import {
  persistBlocks,
  persistBlocksNow,
  persistSessions,
} from "./persistence";
import {
  buildContextUsage,
  touchSession,
} from "./session-utils";
import {
  applyLegacyUsageToLedger,
  applyProviderUsageToLedger,
  contextUsageFromLedger,
  sameUsageCost,
} from "./usage-ledger";
import type { AppStore } from "./types";
import { invalidateEcosystemQueries } from "./ecosystem-events";
import { upsertRecoveryNotice } from "./recovery-notices";
import { clearStaleSessionHealthAlerts, upsertHealthAlert } from "./health-alerts";
import {
  applyLoopRuntimeUpdate,
  applySubagentRuntimeEvent,
} from "./runtime-projections";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;

const CHUNK_TYPES = [
  "thinking_chunk",
  "text_chunk",
  "shell_output",
];

const END_TYPES = [
  "thinking_end",
  "text_end",
  "shell_end",
  "tool_call_end",
];

export function createOutputEventDispatcher(set: StoreSet, get: StoreGet) {
  return (event: StreamEvent) => {
    const { session_id, event_type } = event;

    // Phase 1.7: recovery notice — handled before session lookup so it works
    // even when no session is active (e.g. at startup before restore completes).
    if (event_type === "recovery_notice") {
      const notice = event as Extract<StreamEvent, { event_type: "recovery_notice" }>;
      set({
        recoveryNotices: upsertRecoveryNotice(get().recoveryNotices, notice),
      });
      return;
    }

    // Phase 2: diagnostics_update — no-op for now; future UI panels
    // will consume these to surface runtime health in a diagnostics panel.
    // We return early to avoid falling through to block creation.
    if (event_type === "diagnostics_update") {
      return;
    }

    // Phase 2: health_alert — handled globally before session lookup so
    // watchdog alerts surface even without an active session. Deduped by alert_id.
    if (event_type === "health_alert") {
      const ha = event as Extract<StreamEvent, { event_type: "health_alert" }>;
      const alert: import("./types").RuntimeHealthAlert = {
        alert_id: ha.alert_id,
        session_id: ha.session_id,
        level: ha.level as "info" | "warn" | "critical",
        title: ha.title,
        message: ha.message,
        remediation: ha.remediation ?? null,
      };
      set({
        healthAlerts: upsertHealthAlert(get().healthAlerts, alert),
      });
      return;
    }

    const currentHealthAlerts = get().healthAlerts;
    const freshHealthAlerts = clearStaleSessionHealthAlerts(currentHealthAlerts, session_id);
    if (freshHealthAlerts !== currentHealthAlerts) {
      set({ healthAlerts: freshHealthAlerts });
    }

    if (event_type === "workflow_updated") {
      get().setWorkflowState(session_id, event.state);
      return;
    }

    if (event_type === "agent_turn_updated") {
      const agentTurnBySession = new Map(get().agentTurnBySession);
      agentTurnBySession.set(session_id, event.state);
      set({ agentTurnBySession });
      return;
    }

    if (event_type === "agent_a2a_updated") {
      const agentA2ABySession = new Map(get().agentA2ABySession);
      agentA2ABySession.set(session_id, event.state);
      set({ agentA2ABySession });
      return;
    }

    if (event_type === "subagent_runtime_event") {
      set({
        subagentRuntimeByTask: applySubagentRuntimeEvent(get().subagentRuntimeByTask, event),
      });
      return;
    }

    if (event_type === "loop_runtime_updated") {
      set({
        loopRuntimeByTask: applyLoopRuntimeUpdate(get().loopRuntimeByTask, event),
      });
      return;
    }

    if (event_type === "ecosystem_changed") {
      invalidateEcosystemQueries(queryClient);
      return;
    }

    if (event_type === "delivery_summary") {
      const sessionBlocks = get().sessions.get(session_id)?.blocks ?? [];
      const shouldDedupeReplay = isSameAsLastDeliveryBlock(sessionBlocks, event.summary);
      if (shouldDedupeReplay) {
        return;
      }
      const deliverySummaryBySession = new Map(get().deliverySummaryBySession);
      deliverySummaryBySession.set(session_id, event.summary);
      set({ deliverySummaryBySession });
      persistSessions(get().sessions, get().workflowBySession, deliverySummaryBySession);
    }

    if (event_type === "memory_selection") {
      const selectedContextBySession = new Map(get().selectedContextBySession);
      selectedContextBySession.set(session_id, event.selected);
      set({ selectedContextBySession });
      return;
    }

    if (event_type === "memory_candidate" || event_type === "memory_updated") {
      get().upsertMemory(event.memory);
      return;
    }

    if (event_type === "forge_wiki_context_selected") {
      get().setForgeWikiContext(session_id, event.selected);
      return;
    }

    if (event_type === "mcp_context_status") {
      const mcpContextStatusBySession = new Map(get().mcpContextStatusBySession);
      const current = new Map(mcpContextStatusBySession.get(session_id) ?? []);
      current.set(event.source_id, {
        source_id: event.source_id,
        status: event.status,
        message: event.message ?? null,
      });
      mcpContextStatusBySession.set(session_id, current);
      set({ mcpContextStatusBySession });
      return;
    }

    if (event_type === "forge_wiki_update_proposed" || event_type === "forge_wiki_updated") {
      get().upsertForgeWikiProposal(session_id, event.proposal);
      return;
    }

    const sessions = new Map(get().sessions);
    let session = sessions.get(session_id);

    if (!session) {
      if (event_type === "session_started") {
        const se = event as Extract<StreamEvent, { event_type: "session_started" }>;
        const now = Date.now();
        session = {
          id: session_id,
          agentType: se.agent_type,
          model: se.model,
          workingDir: get().activeWorkspaceId,
          workspaceId: get().activeWorkspaceId,
          createdAt: now,
          updatedAt: now,
          contextWindowTokens: se.context_window_tokens ?? getModelContextWindow(se.model),
          status: "running",
          blocks: [],
          costUsd: 0,
          contextUsage: null,
          usageLedger: null,
          streaming: false,
        };
        sessions.set(session_id, session);
        set({ sessions });
        persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
        return;
      }
      return;
    }

    let blocks = [...session.blocks];

    if ((event_type as string) !== "pending" && event_type !== "session_started"
        && event_type !== "session_status" && event_type !== "session_stopped") {
      blocks = blocks.filter((block) => block.event_type !== "pending");
    }

    if (event_type === "session_started") {
      const startedEvent = event as Extract<StreamEvent, { event_type: "session_started" }>;
      const contextWindowTokens = startedEvent.context_window_tokens ?? getModelContextWindow(startedEvent.model);
      sessions.set(session_id, {
        ...session,
        agentType: startedEvent.agent_type,
        model: startedEvent.model,
        workingDir: session.workingDir ?? get().activeWorkspaceId,
        workspaceId: session.workspaceId ?? get().activeWorkspaceId,
        contextWindowTokens,
        contextUsage: session.contextUsage
          ? buildContextUsage(
              session.contextUsage.usedTokens,
              contextWindowTokens,
              session.contextUsage.source,
              session.contextUsage,
            )
          : null,
        status: "running",
        streaming: false,
        updatedAt: Date.now(),
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      return;
    }

    if (event_type === "session_stopped") {
      blocks = closeInterruptedConfirmBlocks(blocks, "session_stopped");
      sessions.set(session_id, {
        ...session,
        status: "stopped",
        blocks,
        streaming: false,
        updatedAt: Date.now(),
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      persistBlocksNow(session_id, blocks);
      return;
    }

    if (event_type === "usage") {
      const ue = event as Extract<StreamEvent, { event_type: "usage" }>;
      const contextWindowTokens = session.contextWindowTokens ?? getModelContextWindow(session.model);
      const previousLedger = session.usageLedger ?? null;
      const usageLedger = applyLegacyUsageToLedger(previousLedger, ue);
      const isDuplicateLegacy = previousLedger?.lastEventType === "provider_usage"
        && usageLedger.legacyDuplicateIgnored
        && usageLedger.lastEventType === "provider_usage";
      const contextUsage = isDuplicateLegacy
        ? session.contextUsage
        : contextUsageFromLedger(usageLedger, contextWindowTokens, session.contextUsage);
      const costDelta = isDuplicateLegacy ? null : usageLedger.costUsd;
      sessions.set(session_id, touchSession(session, {
        costUsd: addKnownCost(session.costUsd, costDelta),
        contextUsage,
        usageLedger,
        blocks,
      }));
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      return;
    }

    if (event_type === "provider_usage") {
      const providerEvent = event as Extract<StreamEvent, { event_type: "provider_usage" }>;
      const previousLedger = session.usageLedger ?? null;
      const contextWindowTokens = session.contextWindowTokens ?? getModelContextWindow(session.model);
      const newBlock = eventToBlock(event);
      const existingProviderBlockIndex = newBlock
        ? blocks.findIndex((block) => block.block_id === newBlock.block_id && block.event_type === "provider_usage")
        : -1;
      const isProviderReplay = Boolean(
        newBlock &&
        (previousLedger?.lastProviderUsageBlockId === newBlock.block_id || existingProviderBlockIndex >= 0),
      );
      const usageLedger = isProviderReplay && previousLedger
        ? previousLedger
        : applyProviderUsageToLedger(previousLedger, providerEvent);
      const contextUsage = isProviderReplay && session.contextUsage
        ? session.contextUsage
        : contextUsageFromLedger(usageLedger, contextWindowTokens, session.contextUsage);
      const shouldRestoreMissingReplayCost = isProviderReplay
        && !previousLedger
        && existingProviderBlockIndex >= 0
        && session.costUsd <= 0;
      if (newBlock) {
        if (existingProviderBlockIndex >= 0) {
          blocks[existingProviderBlockIndex] = newBlock;
        } else {
          blocks.push(newBlock);
        }
      }
      const costDelta = (isProviderReplay && !shouldRestoreMissingReplayCost)
        || isLegacyProviderCompanion(previousLedger, usageLedger)
        ? null
        : usageLedger.costUsd;
      sessions.set(session_id, touchSession(session, {
        blocks,
        usageLedger,
        contextUsage,
        costUsd: addKnownCost(session.costUsd, costDelta),
      }));
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "session_status") {
      const statusEvent = event as Extract<StreamEvent, { event_type: "session_status" }>;
      let status: SessionState["status"];
      if (statusEvent.status === "error") {
        status = "error";
      } else if (statusEvent.status === "resuming") {
        status = "resuming";
      } else {
        status = "running";
      }
      sessions.set(session_id, {
        ...session,
        status,
        blocks,
        streaming: statusEvent.status === "working",
        updatedAt: Date.now(),
      });
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    if (event_type === "error") {
      const errorEvent = event as Extract<StreamEvent, { event_type: "error" }>;
      if (errorEvent.code === "missing_api_key") {
        set({
          healthAlerts: upsertHealthAlert(get().healthAlerts, {
            alert_id: `missing-api-key:${session_id}`,
            session_id,
            level: "critical",
            title: "缺少模型密钥",
            message: "当前 provider 没有可用的 API key，agent 无法继续发送请求。",
            remediation: "打开设置 > 模型，添加对应 provider 的 API key 后重试。",
          }),
        });
      }
      if (
        errorEvent.code === "missing_api_key" &&
        blocks.some((block) => block.event_type === "error" && block.metadata?.code === "missing_api_key")
      ) {
        return;
      }
      const newBlocks = [
        ...blocks,
        {
          block_id: errorEvent.block_id,
          event_type: "error",
          content: errorEvent.message,
          metadata: { code: errorEvent.code },
          isComplete: true,
        },
      ];
      sessions.set(session_id, {
        ...session,
        blocks: newBlocks,
        updatedAt: Date.now(),
      });
      set({ sessions });
      persistBlocksNow(session_id, newBlocks);
      return;
    }

    if (event_type === "shell_start") {
      blocks = applyShellStartToBlocks(
        blocks,
        event as Extract<StreamEvent, { event_type: "shell_start" }>,
      );
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "file_io") {
      blocks = applyFileIoToBlocks(
        blocks,
        event as Extract<StreamEvent, { event_type: "file_io" }>,
      );
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "context_compacted" || event_type === "context_compact_skipped") {
      blocks = applyCompactResultToBlocks(
        blocks,
        event as Extract<StreamEvent, { event_type: "context_compacted" }> | Extract<StreamEvent, { event_type: "context_compact_skipped" }>,
      );

      const contextUsage = event_type === "context_compacted"
        ? buildContextUsage(
            (event as Extract<StreamEvent, { event_type: "context_compacted" }>).estimated_tokens_after,
            session.contextWindowTokens ?? getModelContextWindow(session.model),
            "local_estimate",
            session.contextUsage,
            {
              from: (event as Extract<StreamEvent, { event_type: "context_compacted" }>).estimated_tokens_before,
              to: (event as Extract<StreamEvent, { event_type: "context_compacted" }>).estimated_tokens_after,
            },
          )
        : session.contextUsage;

      sessions.set(session_id, touchSession(session, { blocks, contextUsage }));
      set({ sessions });
      if (event_type === "context_compacted") {
        persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      }
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "tool_call_result") {
      const resultEvent = event as Extract<StreamEvent, { event_type: "tool_call_result" }>;
      const existingIdx = findToolResultTargetBlockIndex(blocks, resultEvent.block_id);
      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          content: resultEvent.result,
          isComplete: true,
          metadata: {
            ...blocks[existingIdx].metadata,
            is_error: resultEvent.is_error,
            duration_ms: resultEvent.duration_ms,
            ...interruptedToolResultMetadata(resultEvent.result, resultEvent.is_error),
          },
        };
      } else {
        blocks.push({
          block_id: resultEvent.block_id,
          event_type: "tool_call",
          content: resultEvent.result,
          isComplete: true,
          metadata: {
            is_error: resultEvent.is_error,
            duration_ms: resultEvent.duration_ms,
            tool_name: "Tool",
            ...interruptedToolResultMetadata(resultEvent.result, resultEvent.is_error),
          },
        });
      }
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    // Phase 1.6: dedupe tool_call_start in live dispatch — if a block with
    // the same block_id already exists (e.g. from transcript load), update
    // its tool_name/tool_input metadata instead of appending a duplicate.
    if (event_type === "tool_call_start") {
      const tsEvent = event as Extract<StreamEvent, { event_type: "tool_call_start" }>;
      const existingIdx = blocks.findIndex((block) => block.block_id === tsEvent.block_id);
      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          event_type: "tool_call",
          metadata: {
            ...blocks[existingIdx].metadata,
            tool_name: tsEvent.tool_name,
            tool_input: tsEvent.tool_input,
          },
        };
      } else {
        const newBlock = eventToBlock(event);
        if (newBlock) blocks.push(newBlock);
      }
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    if (CHUNK_TYPES.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = event_type === "shell_output"
        ? findShellTargetBlockIndex(blocks, blockIdEvent.block_id)
        : blocks.findIndex((block) => block.block_id === blockIdEvent.block_id);
      const content = "content" in event ? (event as { content: string }).content : "";

      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          content: blocks[existingIdx].content + content,
        };
      } else {
        const blockType = event_type === "thinking_chunk" ? "thinking"
          : event_type === "shell_output" ? "shell"
          : "text";
        blocks.push({
          block_id: blockIdEvent.block_id,
          event_type: blockType,
          content,
          isComplete: false,
          metadata: {},
        });
      }
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    if (END_TYPES.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = event_type === "shell_end"
        ? findShellTargetBlockIndex(blocks, blockIdEvent.block_id)
        : blocks.findIndex((block) => block.block_id === blockIdEvent.block_id);
      if (existingIdx >= 0) {
        if (event_type !== "tool_call_end") {
          blocks[existingIdx] = { ...blocks[existingIdx], isComplete: true };
        }
        if (event_type === "shell_end") {
          const se = event as Extract<StreamEvent, { event_type: "shell_end" }>;
          blocks[existingIdx] = {
            ...blocks[existingIdx],
            metadata: { ...blocks[existingIdx].metadata, exit_code: se.exit_code },
          };
        }
      }
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    // Phase 1.5: dedupe replayed confirm_ask — if a replayed confirm_ask
    // arrives with a block_id already present, replace instead of appending.
    if (event_type === "confirm_ask" && (event as { replayed_interrupted?: boolean }).replayed_interrupted) {
      const newBlock = eventToBlock(event);
      if (newBlock) {
        const existingIdx = blocks.findIndex((block) => block.block_id === newBlock.block_id);
        if (existingIdx >= 0) {
          blocks[existingIdx] = newBlock;
        } else {
          blocks.push(newBlock);
        }
        sessions.set(session_id, touchSession(session, { blocks }));
        set({ sessions });
        persistBlocks(session_id, blocks);
      }
      return;
    }

    if (event_type === "confirm_response") {
      blocks = applyConfirmResponseToBlocks(
        blocks,
        event as Extract<StreamEvent, { event_type: "confirm_response" }>,
      );
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    const newBlock = eventToBlock(event);
    if (newBlock) {
      blocks.push(newBlock);
    }

    sessions.set(session_id, touchSession(session, { blocks, contextUsage: session.contextUsage }));
    set({ sessions });
    persistBlocks(session_id, blocks);
  };
}

function addKnownCost(current: number, delta: number | null | undefined): number {
  return typeof delta === "number" && Number.isFinite(delta)
    ? current + delta
    : current;
}

function isLegacyProviderCompanion(
  previous: SessionState["usageLedger"],
  next: NonNullable<SessionState["usageLedger"]>,
): boolean {
  if (!previous || previous.lastEventType !== "usage") return false;
  if (previous.inputTokens !== next.inputTokens) return false;
  if (previous.outputTokens !== next.outputTokens) return false;
  return sameUsageCost(previous.costUsd, next.costUsd);
}
