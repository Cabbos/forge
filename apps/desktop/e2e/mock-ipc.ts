/**
 * Mock Tauri IPC layer for Playwright E2E tests.
 * Injects before the app mounts — `window.__TAURI_INTERNALS__.invoke` is hijacked.
 */

import type { StreamEvent } from "../src/lib/protocol";

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
}

export function createMockIPC(handlers: MockIPCHandlers = {}) {
  return async (cmd: string, args: Record<string, unknown>) => {
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
      default:
        return undefined;
    }
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
  const blockId = crypto.randomUUID();
  return [
    { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
    // Thinking
    { event_type: "thinking_start", session_id: sessionId, block_id: blockId },
    { event_type: "thinking_chunk", session_id: sessionId, block_id, content: "Let me think about this..." },
    { event_type: "thinking_end", session_id: sessionId, block_id },
    // Text
    { event_type: "text_start", session_id: sessionId, block_id: crypto.randomUUID() },
    { event_type: "text_chunk", session_id: sessionId, block_id: crypto.randomUUID(), content: "I'll create a fibonacci function." },
    { event_type: "text_end", session_id: sessionId, block_id: crypto.randomUUID() },
    // Tool call
    { event_type: "tool_call_start", session_id: sessionId, block_id: crypto.randomUUID(), tool_name: "write_to_file", tool_input: { path: "test.py", content: "def fib..." } },
    { event_type: "tool_call_end", session_id: sessionId, block_id: crypto.randomUUID() },
    { event_type: "tool_call_result", session_id: sessionId, block_id: crypto.randomUUID(), result: "File written: test.py", is_error: false, duration_ms: 150 },
    // Shell
    { event_type: "shell_start", session_id: sessionId, block_id: crypto.randomUUID(), command: "python test.py" },
    { event_type: "shell_output", session_id: sessionId, block_id: crypto.randomUUID(), content: "0 1 1 2 3 5 8" },
    { event_type: "shell_end", session_id: sessionId, block_id: crypto.randomUUID(), exit_code: 0 },
    // Final text
    { event_type: "text_start", session_id: sessionId, block_id: crypto.randomUUID() },
    { event_type: "text_chunk", session_id: sessionId, block_id: crypto.randomUUID(), content: "The fibonacci function works correctly." },
    { event_type: "text_end", session_id: sessionId, block_id: crypto.randomUUID() },
    { event_type: "usage", session_id: sessionId, input_tokens: 120, output_tokens: 45, estimated_cost_usd: 0.001 },
  ];
}
