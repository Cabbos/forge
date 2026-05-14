/**
 * Mock Tauri IPC layer for Playwright E2E tests.
 * Injects before the app mounts — `window.__TAURI_INTERNALS__.invoke` is hijacked.
 */

import type {
  ForgeWikiPage,
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  SelectedForgeWikiPage,
  StreamEvent,
  WikiMemory,
} from "../src/lib/protocol";

export interface MockIPCHandlers {
  create_session?: (args: Record<string, unknown>) => unknown;
  send_input?: (args: Record<string, unknown>) => unknown;
  kill_session?: (args: Record<string, unknown>) => unknown;
  list_sessions?: (args: Record<string, unknown>) => unknown;
  list_capabilities?: (args: Record<string, unknown>) => unknown;
  toggle_capability?: (args: Record<string, unknown>) => unknown;
  confirm_response?: (args: Record<string, unknown>) => unknown;
  get_api_key_status?: (args: Record<string, unknown>) => unknown;
  set_api_key?: (args: Record<string, unknown>) => unknown;
  get_default_working_dir?: (args: Record<string, unknown>) => unknown;
  get_project_runtime_status?: (args: Record<string, unknown>) => unknown;
  get_project_checkpoint_status?: (args: Record<string, unknown>) => unknown;
  list_memories?: (args: Record<string, unknown>) => unknown;
  update_memory?: (args: Record<string, unknown>) => unknown;
  forget_memory?: (args: Record<string, unknown>) => unknown;
  pin_memory?: (args: Record<string, unknown>) => unknown;
  get_workflow_state?: (args: Record<string, unknown>) => unknown;
  override_workflow_route?: (args: Record<string, unknown>) => unknown;
  get_forge_wiki_state?: (args: Record<string, unknown>) => unknown;
  init_forge_wiki?: (args: Record<string, unknown>) => unknown;
  list_forge_wiki_pages?: (args: Record<string, unknown>) => unknown;
  read_forge_wiki_page?: (args: Record<string, unknown>) => unknown;
  select_forge_wiki_context?: (args: Record<string, unknown>) => unknown;
  create_forge_wiki_update_proposal?: (args: Record<string, unknown>) => unknown;
  accept_forge_wiki_update_proposal?: (args: Record<string, unknown>) => unknown;
  discard_forge_wiki_update_proposal?: (args: Record<string, unknown>) => unknown;
}

export function createMockIPC(handlers: MockIPCHandlers = {}) {
  let forgeWikiExists = false;
  const forgeWikiProposals = new Map<string, ForgeWikiUpdateProposal>();

  return async (cmd: string, args: Record<string, unknown>) => {
    const workingDir = "/Users/cabbos/project/forge";
    const projectPath = String(args.projectPath ?? workingDir);
    switch (cmd) {
      case "create_session":
        return handlers.create_session?.(args) ?? { session_id: crypto.randomUUID() };
      case "send_input":
        return handlers.send_input?.(args) ?? undefined;
      case "kill_session":
        return handlers.kill_session?.(args) ?? undefined;
      case "list_sessions":
        return handlers.list_sessions?.(args) ?? [];
      case "list_capabilities":
        return handlers.list_capabilities?.(args) ?? [
          { id: "read_file", name: "File Reader", description: "Read files", kind: "tool", source: "builtin", version: "1.0", enabled: true },
          { id: "code-review", name: "Code Review", description: "Review code", kind: "skill", source: "github", version: "1.2", enabled: true },
        ];
      case "toggle_capability":
        return handlers.toggle_capability?.(args) ?? undefined;
      case "confirm_response":
        return handlers.confirm_response?.(args) ?? undefined;
      case "get_api_key_status":
        return handlers.get_api_key_status?.(args) ?? [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
      case "set_api_key":
        return handlers.set_api_key?.(args) ?? undefined;
      case "get_default_working_dir":
        return handlers.get_default_working_dir?.(args) ?? workingDir;
      case "get_project_runtime_status":
        return handlers.get_project_runtime_status?.(args) ?? {
          working_dir: workingDir,
          has_package_json: true,
          package_manager: "npm",
          dev_script: "dev",
          command: "npm run dev",
          port: 1420,
          url: "http://localhost:1420",
          running: false,
          managed: false,
          pid: null,
          can_start: true,
          can_stop: false,
          can_open: true,
          message: "Preview not running",
          logs: [],
        };
      case "get_project_checkpoint_status":
        return handlers.get_project_checkpoint_status?.(args) ?? {
          working_dir: workingDir,
          is_git_repo: true,
          dirty: false,
          last_checkpoint: null,
          message: "No checkpoint yet",
        };
      case "list_memories":
        return handlers.list_memories?.(args) ?? [];
      case "update_memory":
        return handlers.update_memory?.(args) ?? applyMemoryPatch(args);
      case "forget_memory":
        return handlers.forget_memory?.(args) ?? applyMemoryPatch({ ...args, patch: { status: "forgotten" } });
      case "pin_memory":
        return handlers.pin_memory?.(args) ?? applyMemoryPatch({ ...args, patch: { status: "pinned" } });
      case "get_workflow_state":
        return handlers.get_workflow_state?.(args) ?? null;
      case "override_workflow_route":
        return handlers.override_workflow_route?.(args) ?? {
          session_id: String(args.sessionId ?? "session"),
          route: args.action === "debug" ? "recovery" : args.action === "verify" ? "verification" : args.action === "plan_first" ? "workflow" : "direct",
          phase: args.action === "debug" ? "debugging" : args.action === "verify" ? "verifying" : args.action === "plan_first" ? "clarifying" : "idle",
          beginner_label: args.action === "debug" ? "遇到问题，正在排查" : args.action === "verify" ? "正在检查结果" : args.action === "plan_first" ? "先梳理想法" : "直接回答",
          developer_label: String(args.action ?? "direct"),
          matched_signals: ["manual override"],
          reason: "用户手动切换了当前工作方式。",
          gate: "none",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        };
      case "get_forge_wiki_state":
        return handlers.get_forge_wiki_state?.(args) ?? forgeWikiState(projectPath, forgeWikiExists);
      case "init_forge_wiki":
        if (handlers.init_forge_wiki) return handlers.init_forge_wiki(args);
        forgeWikiExists = true;
        return forgeWikiState(projectPath, true);
      case "list_forge_wiki_pages":
        return handlers.list_forge_wiki_pages?.(args) ?? (forgeWikiExists ? forgeWikiPages(projectPath) : []);
      case "read_forge_wiki_page":
        return handlers.read_forge_wiki_page?.(args) ?? forgeWikiPageContent(String(args.pagePath ?? "index.md"));
      case "select_forge_wiki_context":
        return handlers.select_forge_wiki_context?.(args) ?? forgeWikiSelectedContext(projectPath);
      case "create_forge_wiki_update_proposal": {
        if (handlers.create_forge_wiki_update_proposal) return handlers.create_forge_wiki_update_proposal(args);
        const proposal = forgeWikiProposal(projectPath, args);
        forgeWikiProposals.set(proposal.id, proposal);
        return proposal;
      }
      case "accept_forge_wiki_update_proposal": {
        if (handlers.accept_forge_wiki_update_proposal) return handlers.accept_forge_wiki_update_proposal(args);
        const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
        const accepted = { ...proposal, status: "accepted" as const };
        forgeWikiProposals.set(accepted.id, accepted);
        return accepted;
      }
      case "discard_forge_wiki_update_proposal": {
        if (handlers.discard_forge_wiki_update_proposal) return handlers.discard_forge_wiki_update_proposal(args);
        const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
        const discarded = { ...proposal, status: "discarded" as const };
        forgeWikiProposals.set(discarded.id, discarded);
        return discarded;
      }
      default:
        return undefined;
    }
  };
}

function forgeWikiState(projectPath: string, exists: boolean): ForgeWikiState {
  return {
    project_path: projectPath,
    exists,
    wiki_dir: `${projectPath}/.forge/wiki`,
    pages: exists ? forgeWikiPages(projectPath) : [],
    message: exists ? "项目记录已就绪。" : "还没有项目记录",
  };
}

function forgeWikiPages(projectPath: string): ForgeWikiPage[] {
  return [
    forgeWikiPage(projectPath, "index", "index.md", "项目概览", "index", "项目目标、边界和当前结构。"),
    forgeWikiPage(projectPath, "tasks", "tasks.md", "当前任务", "tasks", "当前任务、验收步骤和后续事项。"),
    forgeWikiPage(projectPath, "decisions", "decisions.md", "决策记录", "decisions", "重要方案和取舍。"),
  ];
}

function forgeWikiPage(
  projectPath: string,
  id: string,
  path: string,
  title: string,
  kind: ForgeWikiPage["kind"],
  summary: string,
): ForgeWikiPage {
  return {
    id,
    project_path: projectPath,
    path,
    title,
    kind,
    summary,
    updated_at: "2026-05-13T00:00:00.000Z",
    token_estimate: 120,
  };
}

function forgeWikiPageContent(pagePath: string): string {
  if (pagePath === "tasks.md") return "# 当前任务\n\n覆盖工作台上下文面板。";
  if (pagePath === "decisions.md") return "# 决策记录\n\n保留关键方案。";
  return "# 项目概览\n\n项目记录预览。";
}

function forgeWikiSelectedContext(projectPath: string): SelectedForgeWikiPage[] {
  return [
    {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks",
      summary: "当前任务、验收步骤和后续事项。",
      score: 0.96,
      reason: "和当前请求最相关",
      injected: true,
    },
    {
      page_id: "index",
      title: "项目概览",
      path: "index.md",
      kind: "index",
      summary: `项目路径：${projectPath}`,
      score: 0.82,
      reason: "提供项目背景",
      injected: true,
    },
  ];
}

function forgeWikiProposal(projectPath: string, args: Record<string, unknown>): ForgeWikiUpdateProposal {
  return {
    id: String(args.proposalId ?? args.id ?? "forge-wiki-proposal"),
    project_path: projectPath,
    session_id: typeof args.sessionId === "string" ? args.sessionId : null,
    target_pages: Array.isArray(args.targetPages) ? args.targetPages.map(String) : ["tasks.md"],
    title: String(args.title ?? "记录项目进展"),
    summary: String(args.summary ?? "补充本轮任务产生的项目记录。"),
    patch_preview: typeof args.patchPreview === "string" ? args.patchPreview : null,
    status: "pending",
    created_at: "2026-05-13T00:00:00.000Z",
  };
}

function applyMemoryPatch(args: Record<string, unknown>): WikiMemory {
  const patch = (args.patch ?? {}) as Partial<WikiMemory>;
  const now = new Date().toISOString();
  return {
    id: String(args.memoryId ?? "memory"),
    category: "project_fact",
    scope: "project",
    status: "accepted",
    title: "背景记录",
    body: "",
    project_path: null,
    source_session_id: null,
    source_message_ids: [],
    confidence: 1,
    created_at: now,
    updated_at: now,
    last_used_at: null,
    use_count: 0,
    tags: [],
    ...patch,
  };
}

/** Simulate streaming events from the backend. */
export function simulateStream(
  page: import("@playwright/test").Page,
  sessionId: string,
  events: StreamEvent[],
  delayMs = 50,
) {
  return page.evaluate(
    ({ sessionId, events, delayMs }) => {
      return new Promise<void>((resolve) => {
        let i = 0;
        const timer = setInterval(() => {
          if (i >= events.length) {
            clearInterval(timer);
            resolve();
            return;
          }
          const event = events[i];
          // @ts-expect-error Tauri listener
          const listeners = window.__tauriListeners?.["session-output"] ?? [];
          for (const fn of listeners) {
            fn({ payload: { ...event, session_id: sessionId } });
          }
          i++;
        }, delayMs);
      });
    },
    { sessionId, events, delayMs },
  );
}

/** A complete conversation: user message + AI thinking + text + tool call + tool result + final text */
export function fullConversation(sessionId: string): StreamEvent[] {
  const thinkingId = crypto.randomUUID();
  const introTextId = crypto.randomUUID();
  const toolId = crypto.randomUUID();
  const shellId = crypto.randomUUID();
  const finalTextId = crypto.randomUUID();
  return [
    { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
    // Thinking
    { event_type: "thinking_start", session_id: sessionId, block_id: thinkingId },
    { event_type: "thinking_chunk", session_id: sessionId, block_id: thinkingId, content: "Let me think about this..." },
    { event_type: "thinking_end", session_id: sessionId, block_id: thinkingId },
    // Text
    { event_type: "text_start", session_id: sessionId, block_id: introTextId },
    { event_type: "text_chunk", session_id: sessionId, block_id: introTextId, content: "I'll create a fibonacci function." },
    { event_type: "text_end", session_id: sessionId, block_id: introTextId },
    // Tool call
    { event_type: "tool_call_start", session_id: sessionId, block_id: toolId, tool_name: "write_to_file", tool_input: { path: "test.py", content: "def fib..." } },
    { event_type: "tool_call_end", session_id: sessionId, block_id: toolId },
    { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "File written: test.py", is_error: false, duration_ms: 150 },
    // Shell
    { event_type: "shell_start", session_id: sessionId, block_id: shellId, command: "python test.py" },
    { event_type: "shell_output", session_id: sessionId, block_id: shellId, content: "0 1 1 2 3 5 8" },
    { event_type: "shell_end", session_id: sessionId, block_id: shellId, exit_code: 0 },
    // Final text
    { event_type: "text_start", session_id: sessionId, block_id: finalTextId },
    { event_type: "text_chunk", session_id: sessionId, block_id: finalTextId, content: "The fibonacci function works correctly." },
    { event_type: "text_end", session_id: sessionId, block_id: finalTextId },
    { event_type: "usage", session_id: sessionId, input_tokens: 120, output_tokens: 45, estimated_cost_usd: 0.001 },
  ];
}
