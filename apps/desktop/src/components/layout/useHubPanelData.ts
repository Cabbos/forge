import { useCallback, useEffect, useMemo, useState } from "react";
import { useActiveBlocks, useActiveWorkspace, useStore } from "@/store";
import { deriveProjectArchiveOverview } from "@/lib/project-archive-overview";
import { getActiveContextItems } from "@/lib/context-activation";
import { type McpContextSources } from "@/lib/tauri";
import { useProjectRuntimeStatusQuery } from "@/hooks/queries/useProjectRuntimeStatusQuery";
import { useMcpContextSourcesQuery } from "@/hooks/queries/useMcpContextSourcesQuery";
import {
  buildContextMaterials,
  type ContextFile,
} from "./archive/contextMaterialMapper";

export type HubPanelSection = "agents" | "records";

const contextFiles: ContextFile[] = [];
const emptyMcpContextSources: McpContextSources = { resources: [], prompts: [] };

interface UseHubPanelDataOptions {
  initialSection?: HubPanelSection | null;
  open: boolean;
}

export function useHubPanelData({ initialSection, open }: UseHubPanelDataOptions) {
  const [recordsRequestedOpen, setRecordsRequestedOpen] = useState(false);
  const activeWorkspace = useActiveWorkspace();
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const { data: runtimeStatus } = useProjectRuntimeStatusQuery(activeId, activeWorkspace?.path, open && !!activeId);
  const projectPath = runtimeStatus?.working_dir ?? activeWorkspace?.path ?? null;
  const { data: mcpContextSources = emptyMcpContextSources } = useMcpContextSourcesQuery(activeId, open && !!activeId);
  const workflow = useStore((s) => activeId ? s.workflowBySession.get(activeId) ?? null : null);
  const agentA2A = useStore((s) => activeId ? s.agentA2ABySession.get(activeId) ?? null : null);
  const firstLoopDraft = useStore((s) => activeId ? s.firstLoopDraftBySession.get(activeId) ?? null : null);
  const deliverySummary = useStore((s) => activeId ? s.deliverySummaryBySession.get(activeId) ?? null : null);
  const selectedMemories = useStore((s) => activeId ? s.selectedContextBySession.get(activeId) ?? [] : []);
  const selectedWikiPages = useStore((s) => activeId ? s.forgeWikiContextBySession.get(activeId) ?? [] : []);
  const selectedMcpContext = useStore((s) => activeId ? s.mcpContextBySession.get(activeId) ?? [] : []);
  const mcpContextStatus = useStore((s) => activeId ? s.mcpContextStatusBySession.get(activeId) ?? null : null);
  const toggleMcpContext = useStore((s) => s.toggleMcpContext);
  const session = activeId ? sessions.get(activeId) : null;
  const blocks = useActiveBlocks();
  const activeContextItems = getActiveContextItems(selectedMemories, selectedWikiPages, selectedMcpContext);
  const contextMaterials = useMemo(
    () => buildContextMaterials(contextFiles, mcpContextSources, selectedMcpContext, mcpContextStatus),
    [mcpContextSources, selectedMcpContext, mcpContextStatus],
  );
  const overview = useMemo(() => deriveProjectArchiveOverview({
    workspace: activeWorkspace,
    session: session ?? null,
    blocks,
    firstLoopDraft,
    deliverySummary,
  }), [activeWorkspace, blocks, deliverySummary, firstLoopDraft, session]);
  const onToggleContext = useCallback((selection: Parameters<typeof toggleMcpContext>[1]) => {
    if (activeId) toggleMcpContext(activeId, selection);
  }, [activeId, toggleMcpContext]);

  useEffect(() => {
    if (open && initialSection === "records") setRecordsRequestedOpen(true);
  }, [initialSection, open]);

  return {
    activeContextItems,
    activeId,
    agentA2A,
    contextMaterials,
    deliverySummary,
    firstLoopDraft,
    overview,
    projectPath,
    recordsRequestedOpen,
    workflow,
    onToggleContext,
  };
}
