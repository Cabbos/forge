import type {
  AgentTurnProjection,
  BlockState,
  ContextUsageState,
  DeliverySummary,
  ForgeWikiUpdateProposal,
  McpContextStatus,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  SessionState,
  StreamEvent,
  WikiMemory,
  WorkflowState,
} from "../lib/protocol";
import type { FirstLoopDraft } from "../lib/first-loop";
import type { McpContextSelection } from "../lib/tauri";
import type { ProviderId } from "../lib/providers";
import type { Workspace } from "../lib/workspaces";

export interface AppStore {
  sessions: Map<string, SessionState>;
  activeSessionId: string | null;
  hydrated: boolean;
  workspaces: Map<string, Workspace>;
  activeWorkspaceId: string | null;
  memories: WikiMemory[];
  selectedContextBySession: Map<string, SelectedContextMemory[]>;
  forgeWikiContextBySession: Map<string, SelectedForgeWikiPage[]>;
  mcpContextBySession: Map<string, McpContextSelection[]>;
  mcpContextStatusBySession: Map<string, Map<string, McpContextStatus>>;
  forgeWikiProposalsBySession: Map<string, ForgeWikiUpdateProposal[]>;
  workflowBySession: Map<string, WorkflowState>;
  agentTurnBySession: Map<string, AgentTurnProjection>;
  firstLoopDraftBySession: Map<string, FirstLoopDraft>;
  deliverySummaryBySession: Map<string, DeliverySummary>;
  agentA2ABySession: Map<string, import("../lib/protocol").AgentA2AProjection>;

  selectedProvider: ProviderId;
  setSelectedProvider: (p: string) => void;
  selectedModel: string;
  setSelectedModel: (m: string) => void;

  hydrate: () => Promise<void>;
  setActiveSession: (id: string | null) => void;
  setActiveWorkspace: (id: string | null) => void;
  upsertWorkspace: (workspace: Workspace) => void;
  removeWorkspace: (id: string) => void;
  addSession: (id: string, provider: string, model: string, workingDir?: string | null) => void;
  removeSession: (id: string) => void;
  setMemories: (memories: WikiMemory[]) => void;
  upsertMemory: (memory: WikiMemory) => void;
  setForgeWikiContext: (sessionId: string, selected: SelectedForgeWikiPage[]) => void;
  toggleMcpContext: (sessionId: string, selection: McpContextSelection) => void;
  clearMcpContext: (sessionId: string) => void;
  upsertForgeWikiProposal: (sessionId: string, proposal: ForgeWikiUpdateProposal) => void;
  setWorkflowState: (sessionId: string, workflow: WorkflowState) => void;
  setFirstLoopDraft: (sessionId: string, draft: FirstLoopDraft | null) => void;
  updateSessionStatus: (id: string, status: SessionState["status"]) => void;
  updateBlock: (sessionId: string, blockId: string, patch: Partial<BlockState>) => void;

  dispatchOutputEvent: (event: StreamEvent) => void;
  addUserMessage: (sessionId: string, text: string) => void;

  pendingInput: string;
  setPendingInput: (text: string) => void;

  theme: "light" | "dark";
  setTheme: (theme: "light" | "dark") => void;
}

export interface PersistedSession {
  id: string;
  agentType: string;
  model: string;
  workingDir?: string | null;
  workspaceId?: string | null;
  createdAt?: number | null;
  updatedAt?: number | null;
  contextWindowTokens?: number | null;
  contextUsage?: ContextUsageState | null;
  status: SessionState["status"];
  workflowState?: WorkflowState | null;
  deliverySummary?: DeliverySummary | null;
}
