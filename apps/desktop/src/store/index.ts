import { create } from "zustand";
import { get as idbGet, set as idbSet, del as idbDel } from "idb-keyval";
import type { BlockState, StreamEvent, SessionState } from "../lib/protocol";

interface AppStore {
  // Sessions
  sessions: Map<string, SessionState>;
  activeSessionId: string | null;
  hydrated: boolean;

  // Actions
  hydrate: () => Promise<void>;
  setActiveSession: (id: string | null) => void;
  addSession: (id: string, agentType: string, model: string) => void;
  removeSession: (id: string) => void;
  updateSessionStatus: (id: string, status: SessionState["status"]) => void;

  // Output events
  dispatchOutputEvent: (event: StreamEvent) => void;
  addUserMessage: (sessionId: string, text: string) => void;

  // Input
  pendingInput: string;
  setPendingInput: (text: string) => void;

  // Theme
  theme: "light" | "dark";
  setTheme: (theme: "light" | "dark") => void;
}

const PERSIST_KEY = "tui-to-gui-sessions";
const BLOCKS_PREFIX = "tui-to-gui-blocks:";
const MAX_PERSISTED_BLOCKS = 100;

interface PersistedSession {
  id: string;
  agentType: string;
  model: string;
  status: SessionState["status"];
}

// Save sessions to IndexedDB. Returns a promise so callers can await when needed.
function persistSessions(sessions: Map<string, SessionState>) {
  const data: PersistedSession[] = [];
  sessions.forEach((s) => {
    data.push({
      id: s.id,
      agentType: s.agentType,
      model: s.model,
      status: s.status,
    });
  });
  return idbSet(PERSIST_KEY, data).catch(() => {});
}

// Save blocks for a session to IndexedDB (capped at MAX_PERSISTED_BLOCKS)
function persistBlocks(sessionId: string, blocks: BlockState[]) {
  const capped = blocks.length > MAX_PERSISTED_BLOCKS
    ? blocks.slice(blocks.length - MAX_PERSISTED_BLOCKS)
    : blocks;
  idbSet(BLOCKS_PREFIX + sessionId, capped).catch(() => {});
}

// Load blocks for a session from IndexedDB
async function loadBlocks(sessionId: string): Promise<BlockState[]> {
  try {
    const blocks = await idbGet<BlockState[]>(BLOCKS_PREFIX + sessionId);
    return blocks ?? [];
  } catch {
    return [];
  }
}

export const useStore = create<AppStore>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  hydrated: false,
  pendingInput: "",

  hydrate: async () => {
    try {
      const data = await idbGet<PersistedSession[]>(PERSIST_KEY);
      const savedTheme = await idbGet<string>("tui-theme").catch(() => null);
      if (data && data.length > 0) {
        const sessions = new Map<string, SessionState>();
        for (const s of data) {
          const blocks = await loadBlocks(s.id);
          sessions.set(s.id, { ...s, blocks, costUsd: 0 });
        }
        set({ sessions, hydrated: true, theme: (savedTheme as "light" | "dark") || get().theme });
      } else {
        set({ hydrated: true, theme: (savedTheme as "light" | "dark") || get().theme });
      }
    } catch {
      set({ hydrated: true });
    }
  },
  theme: (typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches)
    ? "dark"
    : "light",

  setActiveSession: (id) => set({ activeSessionId: id }),

  addSession: (id, agentType, model) => {
    const sessions = new Map(get().sessions);
    sessions.set(id, {
      id, agentType, model, status: "running", blocks: [], costUsd: 0,
    });
    set({ sessions, activeSessionId: id });
    persistSessions(sessions);
  },

  removeSession: (id) => {
    const sessions = new Map(get().sessions);
    sessions.delete(id);
    const activeSessionId =
      get().activeSessionId === id ? null : get().activeSessionId;
    set({ sessions, activeSessionId });
    // Await both to prevent races with async persist from other actions
    Promise.all([
      persistSessions(sessions),
      idbDel(BLOCKS_PREFIX + id).catch(() => {}),
    ]).catch(() => {});
  },

  updateSessionStatus: (id, status) => {
    const sessions = new Map(get().sessions);
    const session = sessions.get(id);
    if (session) {
      sessions.set(id, { ...session, status });
    }
    set({ sessions });
    persistSessions(sessions);
  },

  addUserMessage: (sessionId, text) => {
    const sessions = new Map(get().sessions);
    const session = sessions.get(sessionId);
    if (!session) return;
    const blocks = [...session.blocks];
    blocks.push({
      block_id: crypto.randomUUID(),
      event_type: "user_message",
      content: text,
      isComplete: true,
      metadata: {},
    });
    sessions.set(sessionId, { ...session, blocks });
    set({ sessions });
    persistBlocks(sessionId, blocks);
  },

  dispatchOutputEvent: (event) => {
    const { session_id, event_type } = event;
    const sessions = new Map(get().sessions);
    let session = sessions.get(session_id);

    if (!session) {
      // If session_started arrives before addSession, create it from the event
      if (event_type === "session_started") {
        const se = event as Extract<StreamEvent, { event_type: "session_started" }>;
        session = { id: session_id, agentType: se.agent_type, model: se.model, status: "running", blocks: [], costUsd: 0 };
        sessions.set(session_id, session);
        set({ sessions });
        persistSessions(sessions);
        return;
      }
      return;
    }

    const blocks = [...session.blocks];

    // Handle block accumulation for streaming events
    const chunkTypes = [
      "thinking_chunk",
      "text_chunk",
      "shell_output",
    ];

    const endTypes = [
      "thinking_end",
      "text_end",
      "shell_end",
      "tool_call_end",
    ];

    // Session lifecycle events
    if (event_type === "session_started") {
      // Update session info from the backend event
      const startedEvent = event as Extract<StreamEvent, { event_type: "session_started" }>;
      sessions.set(session_id, {
        ...session,
        agentType: startedEvent.agent_type,
        model: startedEvent.model,
      });
      set({ sessions });
      persistSessions(sessions);
      return;
    }

    if (event_type === "session_stopped") {
      sessions.set(session_id, {
        ...session,
        status: "stopped",
        blocks,
      });
      set({ sessions });
      persistSessions(sessions);
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "usage") {
      const ue = event as Extract<StreamEvent, { event_type: "usage" }>;
      sessions.set(session_id, {
        ...session,
        costUsd: (session.costUsd || 0) + ue.estimated_cost_usd,
        blocks,
      });
      set({ sessions });
      persistSessions(sessions);
      return;
    }

    if (event_type === "session_status") {
      const statusEvent = event as Extract<StreamEvent, { event_type: "session_status" }>;
      const status = statusEvent.status === "error" ? "error" : "running";
      sessions.set(session_id, {
        ...session,
        status,
        blocks,
      });
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    if (event_type === "error") {
      const errorEvent = event as Extract<StreamEvent, { event_type: "error" }>;
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
      });
      set({ sessions });
      persistBlocks(session_id, newBlocks);
      return;
    }

    // For tool_call_result, find the tool_call block and merge
    if (event_type === "tool_call_result") {
      const resultEvent = event as Extract<StreamEvent, { event_type: "tool_call_result" }>;
      const existingIdx = blocks.findIndex((b) =>
        b.event_type === "tool_call" && b.block_id === resultEvent.block_id
      );
      if (existingIdx >= 0) {
        // Merge result into tool_call block
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
        // Fallback: create standalone block
        blocks.push({
          block_id: resultEvent.block_id,
          event_type: "tool_call_result",
          content: resultEvent.result,
          isComplete: true,
          metadata: {
            is_error: resultEvent.is_error,
            duration_ms: resultEvent.duration_ms,
            tool_name: "Tool",
          },
        });
      }
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    // For chunk events, find existing block and append content
    if (chunkTypes.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = blocks.findIndex((b) => b.block_id === blockIdEvent.block_id);
      const content = "content" in event ? (event as { content: string }).content : "";

      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          content: blocks[existingIdx].content + content,
        };
      } else {
        // No existing block — create one (handles PTY output that emits chunks without a start event)
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
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    // For end events, mark block as complete
    if (endTypes.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = blocks.findIndex((b) => b.block_id === blockIdEvent.block_id);
      if (existingIdx >= 0) {
        blocks[existingIdx] = { ...blocks[existingIdx], isComplete: true };
        // Capture exit_code for shell blocks
        if (event_type === "shell_end") {
          const se = event as Extract<StreamEvent, { event_type: "shell_end" }>;
          blocks[existingIdx] = {
            ...blocks[existingIdx],
            metadata: { ...blocks[existingIdx].metadata, exit_code: se.exit_code },
          };
        }
      }
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    // For all other events, create a new block
    const newBlock = eventToBlock(event);
    if (newBlock) {
      blocks.push(newBlock);
    }

    sessions.set(session_id, { ...session, blocks });
    set({ sessions });
    persistBlocks(session_id, blocks);
  },

  setPendingInput: (text) => set({ pendingInput: text }),

  setTheme: (theme) => {
    set({ theme });
    idbSet("tui-theme", theme).catch(() => {});
  },
}));

function eventToBlock(event: StreamEvent): BlockState | null {
  const base = {
    block_id: "block_id" in event ? (event as { block_id: string }).block_id : "",
    isComplete: false,
    metadata: {} as Record<string, unknown>,
  };

  switch (event.event_type) {
    case "thinking_start":
      return { ...base, event_type: "thinking", content: "", metadata: {} };
    case "text_start":
      return { ...base, event_type: "text", content: "" };
    case "tool_call_start":
      return {
        ...base,
        event_type: "tool_call",
        content: "",
        metadata: {
          tool_name: event.tool_name,
          tool_input: event.tool_input,
        },
      };
    case "tool_call_result":
      return {
        ...base,
        event_type: "tool_call_result",
        content: event.result,
        metadata: {
          is_error: event.is_error,
          duration_ms: event.duration_ms,
        },
      };
    case "diff_view":
      return {
        ...base,
        event_type: "diff_view",
        content: event.new_content,
        metadata: {
          file_path: event.file_path,
          old_content: event.old_content,
        },
      };
    case "shell_start":
      return {
        ...base,
        event_type: "shell",
        content: "",
        metadata: { command: event.command },
      };
    case "confirm_ask":
      return {
        ...base,
        event_type: "confirm_ask",
        content: event.question,
        metadata: { kind: event.kind },
      };
    default:
      return null;
  }
}

// Selector hooks
export const useActiveSession = () =>
  useStore((s) => {
    if (!s.activeSessionId) return null;
    return s.sessions.get(s.activeSessionId) ?? null;
  });

export const useSessionList = () =>
  useStore((s) => Array.from(s.sessions.values()));

export const useActiveBlocks = () =>
  useStore((s) => {
    if (!s.activeSessionId) return [];
    return s.sessions.get(s.activeSessionId)?.blocks ?? [];
  });
