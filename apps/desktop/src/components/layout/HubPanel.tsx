import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ChevronDown, ChevronRight, ClipboardCheck, FilePlus2, FileText, FolderOpen, Layers3, X } from "lucide-react";
import { useActiveBlocks, useActiveWorkspace, useStore } from "@/store";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ActiveContextSection } from "@/components/context/ActiveContextSection";
import { FirstLoopCard } from "@/components/context/FirstLoopCard";
import { ProjectOverviewCard } from "@/components/context/ProjectOverviewCard";
import { WikiSections } from "@/components/context/WikiSections";
import { ProjectStatusCard } from "./ProjectStatusCard";
import { CurrentTaskCard } from "@/components/workflow/CurrentTaskCard";
import { cn } from "@/lib/utils";
import { getActiveContextItems } from "@/lib/context-activation";
import { deriveProjectArchiveOverview } from "@/lib/project-archive-overview";
import { getProjectRuntimeStatus, listMcpContextSources, type McpContextSources } from "@/lib/tauri";
import type { McpContextPromptArgument, McpContextSelection } from "@/lib/tauri";
import type { DeliverySummary, McpContextStatus } from "@/lib/protocol";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

type ParseStatus = "pending" | "parsed" | "failed" | "available" | "read_failed";

interface ContextFile {
  id: string;
  name: string;
  type: string;
  status: ParseStatus;
  inContext: boolean;
  selection?: McpContextSelection;
  promptArguments?: McpContextPromptArgument[];
  sourceLabel?: string;
  description?: string;
  statusMessage?: string | null;
}

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
              <ProductLayerHeader title="交付" meta="最近状态" />
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

function ArchiveSummaryStrip({
  contextCount,
  deliverySummary,
  overview,
}: {
  contextCount: number;
  deliverySummary: DeliverySummary | null | undefined;
  overview: ReturnType<typeof deriveProjectArchiveOverview>;
}) {
  const recordValue = deliverySummary?.record_status === "pending"
    ? "待确认"
    : deliverySummary?.record_status === "accepted"
      ? "已记录"
      : "未请求";

  return (
    <section data-testid="project-archive-summary-strip" className="forge-archive-summary-strip" aria-label="项目档案摘要">
      <ArchiveSummaryItem
        icon={<FolderOpen className="size-3.5" />}
        label="项目"
        value={overview.projectName}
        title={overview.projectPath}
      />
      <ArchiveSummaryItem
        icon={<Layers3 className="size-3.5" />}
        label="上下文"
        value={contextCount > 0 ? `${contextCount} 个已加入` : "未加入"}
      />
      <ArchiveSummaryItem
        icon={<ClipboardCheck className="size-3.5" />}
        label="记录"
        value={recordValue}
        title={deliverySummary?.checkpoint_label}
      />
    </section>
  );
}

function ArchiveSummaryItem({
  detail,
  icon,
  label,
  title,
  value,
}: {
  detail?: string;
  icon: ReactNode;
  label: string;
  title?: string;
  value: string;
}) {
  return (
    <div className="forge-archive-summary-item" title={title}>
      <span className="forge-archive-summary-icon">{icon}</span>
      <span className="forge-archive-summary-copy">
        <span className="forge-archive-summary-label">{label}</span>
        <span className="forge-archive-summary-value">{value}</span>
        {detail ? <span className="forge-archive-summary-detail">{detail}</span> : null}
      </span>
    </div>
  );
}

function ArchiveDisclosure({
  children,
  defaultOpen = false,
  meta,
  testId,
  title,
}: {
  children: ReactNode;
  defaultOpen?: boolean;
  meta?: string | null;
  testId: string;
  title: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const Icon = open ? ChevronDown : ChevronRight;

  useEffect(() => {
    if (defaultOpen) setOpen(true);
  }, [defaultOpen]);

  return (
    <section data-testid={testId}>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="forge-disclosure-row"
      >
        <span className="flex min-w-0 items-center gap-2">
          <Icon className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="truncate text-[11px] font-medium text-foreground">{title}</span>
        </span>
        {meta && <span className="shrink-0 text-[10px] text-muted-foreground">{meta}</span>}
      </button>
      {open && <div className="mt-2 space-y-3">{children}</div>}
    </section>
  );
}

function ProductLayerHeader({ title, meta }: { title: string; meta?: string | null }) {
  return (
    <div className="flex items-center justify-between pt-1">
      <h3 className="text-[11px] font-semibold text-foreground">{title}</h3>
      {meta && <span className="text-[10px] text-muted-foreground">{meta}</span>}
    </div>
  );
}

function ContextFilesSection({
  files,
  onToggle,
}: {
  files: ContextFile[];
  onToggle: (selection: McpContextSelection) => void;
}) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-end">
        <button
          type="button"
          disabled
          className="forge-action text-muted-foreground disabled:cursor-default disabled:opacity-70"
          title="添加文件"
        >
          <FilePlus2 className="size-3" />
          添加文件
        </button>
      </div>

      <div className="forge-surface overflow-hidden">
        <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] gap-2 border-b border-border px-3 py-2 text-[10px] uppercase tracking-wider text-muted-foreground">
          <span>文件名</span>
          <span>类型</span>
          <span>解析状态</span>
          <span className="text-right">参考</span>
        </div>

        {files.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 px-3 py-8 text-center">
            <FileText className="size-5 text-muted-foreground" />
            <div className="text-xs text-muted-foreground">还没有添加资料</div>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {files.map((file) => (
              <ContextFileRow key={file.id} file={file} onToggle={onToggle} />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function ContextFileRow({
  file,
  onToggle,
}: {
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
}) {
  const content = (
    <>
      <div className="min-w-0">
        <div className="truncate text-foreground">{file.name}</div>
        <div className="mt-0.5 truncate text-[10px] text-muted-foreground">
          {[file.sourceLabel, file.statusMessage].filter(Boolean).join(" · ")}
        </div>
      </div>
      <span className="truncate font-mono text-[10px] text-muted-foreground">{file.type}</span>
      <span className={cn("truncate text-[10px]", statusClass(file.status))}>
        {statusLabel(file.status)}
      </span>
      <span className={cn(
        "text-right text-[10px]",
        file.inContext ? "text-primary" : "text-muted-foreground",
      )}>
        {file.inContext ? "已加入" : "未加入"}
      </span>
    </>
  );

  if (file.selection) {
    const hasPromptArguments =
      file.selection.kind === "prompt" && (file.promptArguments?.length ?? 0) > 0;
    if (hasPromptArguments) {
      return (
        <ContextPromptRow
          content={content}
          file={file}
          onToggle={onToggle}
        />
      );
    }

    return (
      <button
        type="button"
        aria-pressed={file.inContext}
        onClick={() => onToggle(file.selection!)}
        className="grid w-full grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-muted/25"
      >
        {content}
      </button>
    );
  }

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-xs">
      {content}
    </div>
  );
}

function ContextPromptRow({
  content,
  file,
  onToggle,
}: {
  content: ReactNode;
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [values, setValues] = useState<Record<string, string>>({});
  const argumentsList = file.promptArguments ?? [];

  const handleRowClick = () => {
    if (file.inContext) {
      onToggle(file.selection!);
      return;
    }
    setEditing((value) => !value);
  };

  const addPrompt = () => {
    if (!file.selection || file.selection.kind !== "prompt") return;
    onToggle({
      ...file.selection,
      arguments: values,
    });
    setEditing(false);
  };

  return (
    <div>
      <button
        type="button"
        aria-pressed={file.inContext}
        onClick={handleRowClick}
        className="grid w-full grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-muted/25"
      >
        {content}
      </button>
      {editing && !file.inContext ? (
        <div className="space-y-2 border-t border-border/70 px-3 py-2.5">
          {argumentsList.map((argument) => (
            <label key={argument.name} className="block space-y-1">
              <span className="text-[10px] text-muted-foreground">
                {argument.name}{argument.required ? " *" : ""}
              </span>
              <input
                aria-label={argument.name}
                value={values[argument.name] ?? ""}
                onChange={(event) => setValues((current) => ({
                  ...current,
                  [argument.name]: event.target.value,
                }))}
                placeholder={argument.description || argument.name}
                className="h-7 w-full rounded-md border border-border bg-muted/20 px-2 text-xs text-foreground outline-none transition-colors placeholder:text-muted-foreground/45 focus:border-primary/45"
              />
            </label>
          ))}
          <div className="flex justify-end">
            <button
              type="button"
              onClick={addPrompt}
              className="forge-action"
            >
              加入本轮
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function statusLabel(status: ParseStatus) {
  switch (status) {
    case "read_failed":
      return "读取失败";
    case "available":
      return "可用";
    case "pending":
      return "解析中";
    case "parsed":
      return "已解析";
    case "failed":
      return "解析失败";
  }
}

function statusClass(status: ParseStatus) {
  switch (status) {
    case "read_failed":
      return "text-destructive";
    case "available":
      return "text-emerald-400";
    case "pending":
      return "text-primary";
    case "parsed":
      return "text-emerald-400";
    case "failed":
      return "text-destructive";
  }
}

function buildContextMaterials(
  files: ContextFile[],
  sources: McpContextSources,
  selected: McpContextSelection[],
  statuses: Map<string, McpContextStatus> | null,
): ContextFile[] {
  const connectorResources = sources.resources.map((resource): ContextFile => {
    const id = `mcp-resource:${resource.server_id}:${resource.uri}`;
    const selection: McpContextSelection = {
      kind: "resource",
      server_id: resource.server_id,
      uri: resource.uri,
      name: resource.name || resource.uri,
      description: resource.description,
      mime_type: resource.mime_type,
    };
    const status = statuses?.get(id) ?? null;
    return {
      id,
      name: selection.name,
      type: compactResourceType(resource.mime_type),
      status: contextFileStatus(status),
      inContext: selected.some((item) => sameContextSelection(item, selection)),
      selection,
      sourceLabel: `连接资料 · ${resource.server_id}`,
      description: resource.description,
      statusMessage: status?.status === "failed" ? status.message ?? null : null,
    };
  });
  const connectorPrompts = sources.prompts.map((prompt): ContextFile => {
    const id = `mcp-prompt:${prompt.server_id}:${prompt.name}`;
    const selection: McpContextSelection = {
      kind: "prompt",
      server_id: prompt.server_id,
      name: prompt.name,
      description: prompt.description,
    };
    const status = statuses?.get(id) ?? null;
    return {
      id,
      name: prompt.name,
      type: "提示词",
      status: contextFileStatus(status),
      inContext: selected.some((item) => sameContextSelection(item, selection)),
      selection,
      promptArguments: prompt.arguments,
      sourceLabel: `连接提示词 · ${prompt.server_id}`,
      description: prompt.description,
      statusMessage: status?.status === "failed" ? status.message ?? null : null,
    };
  });

  return [...files, ...connectorResources, ...connectorPrompts];
}

function contextFileStatus(status: McpContextStatus | null): ParseStatus {
  if (!status) return "available";
  return status.status === "failed" ? "read_failed" : "parsed";
}

function sameContextSelection(a: McpContextSelection, b: McpContextSelection) {
  if (a.kind !== b.kind || a.server_id !== b.server_id) return false;
  return a.kind === "resource" && b.kind === "resource"
    ? a.uri === b.uri
    : a.kind === "prompt" && b.kind === "prompt" && a.name === b.name;
}

function compactResourceType(mimeType: string | null) {
  if (!mimeType) return "资料";
  if (mimeType.includes("markdown")) return "md";
  if (mimeType.includes("pdf")) return "pdf";
  if (mimeType.includes("json")) return "json";
  if (mimeType.includes("text")) return "txt";
  return "资料";
}
