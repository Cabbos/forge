import { useEffect, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import { useActiveBlocks, useActiveWorkspace, useStore } from "@/store";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ActiveContextSection } from "@/components/context/ActiveContextSection";
import { FirstLoopCard } from "@/components/context/FirstLoopCard";
import { ProjectOverviewCard } from "@/components/context/ProjectOverviewCard";
import { WikiSections } from "@/components/context/WikiSections";
import { ProjectStatusCard } from "./ProjectStatusCard";
import { CurrentTaskCard } from "@/components/workflow/CurrentTaskCard";
import { deriveProjectArchiveOverview } from "@/lib/project-archive-overview";
import { getActiveContextItems } from "@/lib/context-activation";
import { getProjectRuntimeStatus, listMcpContextSources, type McpContextSources } from "@/lib/tauri";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";
import {
  buildContextMaterials,
  type ContextFile,
} from "./archive/contextMaterialMapper";
import { ArchiveDisclosure } from "./archive/ArchiveDisclosure";
import { ArchiveLayerHeader } from "./archive/ArchiveLayerHeader";
import { ArchiveSummaryStrip } from "./archive/ArchiveSummaryStrip";
import { ContextFilesSection } from "./archive/ArchiveContextMaterials";

const contextFiles: ContextFile[] = [];
const emptyMcpContextSources: McpContextSources = { resources: [], prompts: [] };
type HubPanelSection = "records";

interface HubPanelProps {
  open: boolean;
  initialSection?: HubPanelSection | null;
  onOpenChange: (open: boolean) => void;
}

export function HubPanel({ open, initialSection, onOpenChange }: HubPanelProps) {
  const panelRef = useRef<HTMLElement>(null);
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
  const projectOverview = useMemo(() => deriveProjectArchiveOverview({
    workspace: activeWorkspace,
    session: session ?? null,
    blocks,
    firstLoopDraft,
    deliverySummary,
  }), [activeWorkspace, blocks, deliverySummary, firstLoopDraft, session]);

  useGSAP(() => {
    if (!open || prefersReducedMotion()) return;
    const panel = panelRef.current;
    if (!panel) return;

    const sections = gsap.utils.toArray<HTMLElement>("[data-forge-motion='archive-section']", panel);
    const timeline = gsap.timeline();
    timeline.fromTo(
      panel,
      { autoAlpha: 0, x: 18 },
      {
        autoAlpha: 1,
        x: 0,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
    if (sections.length > 0) {
      timeline.fromTo(
        sections,
        { autoAlpha: 0, y: 5 },
        {
          autoAlpha: 1,
          y: 0,
          duration: forgeMotion.evidence.duration,
          ease: forgeMotion.evidence.ease,
          stagger: 0.025,
          clearProps: "transform,opacity,visibility",
        },
        "-=0.08",
      );
    }
  }, { scope: panelRef, dependencies: [open] });

  useEffect(() => {
    if (open && initialSection === "records") setRecordsRequestedOpen(true);
  }, [initialSection, open]);

  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onOpenChange(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onOpenChange, open]);

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

  if (!open) return null;

  return (
    <>
      <aside
        ref={panelRef}
        data-testid="project-archive-panel"
        aria-label="项目档案"
        data-forge-motion="archive-panel"
        className="forge-inspector fixed right-0 top-0 z-50 flex h-full flex-col overflow-hidden"
      >
        <div className="forge-inspector-header">
          <div className="forge-inspector-title-block">
            <span className="forge-inspector-title">项目档案</span>
            <span className="forge-inspector-subtitle">状态、上下文与交付</span>
          </div>
          <button
            type="button"
            aria-label="关闭项目档案"
            onClick={() => onOpenChange(false)}
            className="forge-icon-button"
            title="关闭项目档案"
          >
            <X className="size-4" />
          </button>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div data-testid="project-archive-body" className="forge-inspector-body">
            <div data-forge-motion="archive-section">
              <ArchiveSummaryStrip
                contextCount={activeContextItems.length}
                deliverySummary={deliverySummary}
                overview={projectOverview}
              />
            </div>

            <div data-forge-motion="archive-section">
              <ProjectOverviewCard overview={projectOverview} />
            </div>

            <div data-forge-motion="archive-section">
              <CurrentTaskCard workflow={workflow} />
            </div>

            {firstLoopDraft && (
              <div data-forge-motion="archive-section">
                <FirstLoopCard draft={firstLoopDraft} />
              </div>
            )}

            {activeContextItems.length > 0 && (
              <div data-forge-motion="archive-section">
                <ActiveContextSection items={activeContextItems} />
              </div>
            )}

            <div data-forge-motion="archive-section">
              <ArchiveDisclosure
                testId="archive-disclosure-records"
                title="项目记录"
                meta="记录与建议"
                defaultOpen={recordsRequestedOpen || deliverySummary?.record_status === "pending"}
              >
                <WikiSections sessionId={activeId} projectPath={projectPath} />
              </ArchiveDisclosure>
            </div>

            <div data-forge-motion="archive-section">
              <ArchiveDisclosure
                testId="archive-disclosure-files"
                title="资料"
                meta={contextMaterials.length > 0 ? `${contextMaterials.length} 个资料` : "未添加"}
              >
                <ContextFilesSection
                  files={contextMaterials}
                  onToggle={(selection) => {
                    if (activeId) toggleMcpContext(activeId, selection);
                  }}
                />
              </ArchiveDisclosure>
            </div>

            <div data-forge-motion="archive-section">
              <ArchiveLayerHeader title="交付" meta="最近状态" />
            </div>

            <div data-forge-motion="archive-section">
              {activeId ? (
                <ProjectStatusCard sessionId={activeId} />
              ) : (
                <div className="forge-empty">
                  选择一个任务后查看预览和检查点。
                </div>
              )}
            </div>
          </div>
        </ScrollArea>
      </aside>
    </>
  );
}
