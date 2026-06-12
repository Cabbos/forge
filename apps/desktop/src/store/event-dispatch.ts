import type { StreamEvent } from "../lib/protocol";
import { getModelContextWindow } from "../lib/providers";
import {
  applyCompactResultToBlocks,
  applyShellStartToBlocks,
  closeInterruptedConfirmBlocks,
  eventToBlock,
  findShellTargetBlockIndex,
  findToolResultTargetBlockIndex,
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
import type { AppStore } from "./types";

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
      sessions.set(session_id, {
        ...session,
        costUsd: (session.costUsd || 0) + ue.estimated_cost_usd,
        contextUsage: buildContextUsage(
          ue.input_tokens,
          contextWindowTokens,
          "provider_usage",
          session.contextUsage,
        ),
        blocks,
        updatedAt: Date.now(),
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      return;
    }

    if (event_type === "session_status") {
      const statusEvent = event as Extract<StreamEvent, { event_type: "session_status" }>;
      const status = statusEvent.status === "error" ? "error" : "running";
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
          },
        });
      }
      sessions.set(session_id, touchSession(session, { blocks }));
      set({ sessions });
      persistBlocksNow(session_id, blocks);
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

    const newBlock = eventToBlock(event);
    if (newBlock) {
      blocks.push(newBlock);
    }

    sessions.set(session_id, touchSession(session, { blocks, contextUsage: session.contextUsage }));
    set({ sessions });
    persistBlocks(session_id, blocks);
  };
}
