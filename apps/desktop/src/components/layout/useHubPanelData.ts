import { useCallback, useEffect, useMemo, useState } from "react";
import { useActiveBlocks, useActiveWorkspace, useStore } from "@/store";
import { deriveProjectArchiveOverview } from "@/lib/project-archive-overview";
import { getActiveContextItems } from "@/lib/context-activation";
import { getProjectRuntimeStatus, listMcpContextSources, type McpContextSources } from "@/lib/tauri";
import {
  buildContextMaterials,
  type ContextFile,
} from "./archive/contextMaterialMapper";

export type HubPanelSection = "records";

const contextFiles: ContextFile[] = [];
const emptyMcpContextSources: McpContextSources = { resources: [], prompts: [] };

interface UseHubPanelDataOptions {
  initialSection?: HubPanelSection | null;
  open: boolean;
}

export function useHubPanelData({ initialSection, open }: UseHubPanelDataOptions) {
  const [recordsRequestedOpen, setRecordsRequestedOpen] = useState(false);
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [mcpContextSources, setMcpContextSources] = useState<McpContextSources>(emptyMcpContextSources);
  const activeWorkspace = useActiveWorkspace();
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const workflow = useStore((s) => activeId ? s.workflowBySession.get(activeId) ?? null : null);
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

  useEffect(() => {
    let cancelled = false;

    setProjectPath(activeWorkspace?.path ?? null);
    if (!open || !activeId) return;

    getProjectRuntimeStatus(activeId, activeWorkspace?.path)
      .then((status) => {
        if (!cancelled) setProjectPath(status.working_dir || null);
      })
      .catch(() => {
        if (!cancelled) setProjectPath(null);
      });

    return () => {
      cancelled = true;
    };
  }, [activeId, activeWorkspace?.path, open]);

  useEffect(() => {
    let cancelled = false;

    if (!open || !activeId) {
      setMcpContextSources(emptyMcpContextSources);
      return;
    }

    listMcpContextSources(activeId)
      .then((sources) => {
        if (!cancelled) setMcpContextSources(sources);
      })
      .catch(() => {
        if (!cancelled) setMcpContextSources(emptyMcpContextSources);
      });

    return () => {
      cancelled = true;
    };
  }, [activeId, open]);

  return {
    activeContextItems,
    activeId,
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
