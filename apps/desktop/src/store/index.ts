import { create } from "zustand";
import {
  DEFAULT_PROVIDER_ID,
  getDefaultModel,
} from "../lib/providers";
import type { AppStore } from "./types";
import { createStoreSelectors } from "./selectors";
import { createOutputEventDispatcher } from "./event-dispatch";
import { createHydrateAction } from "./hydration";
import { createWorkspaceActions } from "./workspace-actions";
import { createContextActions } from "./context-actions";
import { createSessionActions } from "./session-actions";
import { createPreferencesActions } from "./preferences-actions";

export const useStore = create<AppStore>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  hydrated: false,
  workspaces: new Map(),
  activeWorkspaceId: null,
  memories: [],
  selectedContextBySession: new Map(),
  forgeWikiContextBySession: new Map(),
  mcpContextBySession: new Map(),
  mcpContextStatusBySession: new Map(),
  forgeWikiProposalsBySession: new Map(),
  workflowBySession: new Map(),
  agentTurnBySession: new Map(),
  firstLoopDraftBySession: new Map(),
  deliverySummaryBySession: new Map(),
  agentA2ABySession: new Map(),
  pendingInput: "",
  selectedProvider: DEFAULT_PROVIDER_ID,
  selectedModel: getDefaultModel(DEFAULT_PROVIDER_ID),

  hydrate: createHydrateAction(set, get),
  theme: (typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches)
    ? "dark"
    : "light",

  ...createWorkspaceActions(set, get),
  ...createContextActions(set, get),
  ...createSessionActions(set, get),
  ...createPreferencesActions(set, get),

  dispatchOutputEvent: createOutputEventDispatcher(set, get),

  setPendingInput: (text) => set({ pendingInput: text }),
}));

export const {
  useActiveSession,
  useSessionList,
  useWorkspaceList,
  useActiveWorkspace,
  useActiveBlocks,
} = createStoreSelectors(useStore);
