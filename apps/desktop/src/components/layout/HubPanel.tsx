import { useEffect, useMemo, useState, type ReactNode } from "react";
import { ChevronDown, ChevronRight, FilePlus2, FileText, X } from "lucide-react";
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
import { getProjectRuntimeStatus } from "@/lib/tauri";

type ParseStatus = "pending" | "parsed" | "failed";

interface ContextFile {
  id: string;
  name: string;
  type: string;
  status: ParseStatus;
  inContext: boolean;
}

const contextFiles: ContextFile[] = [];

export function HubPanel() {
  const [open, setOpen] = useState(false);
  const [recordsRequestedOpen, setRecordsRequestedOpen] = useState(false);
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const activeWorkspace = useActiveWorkspace();
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const workflow = useStore((s) => activeId ? s.workflowBySession.get(activeId) ?? null : null);
  const firstLoopDraft = useStore((s) => activeId ? s.firstLoopDraftBySession.get(activeId) ?? null : null);
  const deliverySummary = useStore((s) => activeId ? s.deliverySummaryBySession.get(activeId) ?? null : null);
  const selectedMemories = useStore((s) => activeId ? s.selectedContextBySession.get(activeId) ?? [] : []);
  const selectedWikiPages = useStore((s) => activeId ? s.forgeWikiContextBySession.get(activeId) ?? [] : []);
  const session = activeId ? sessions.get(activeId) : null;
  const blocks = useActiveBlocks();
  const activeContextItems = getActiveContextItems(selectedMemories, selectedWikiPages);
  const projectOverview = useMemo(() => deriveProjectArchiveOverview({
    workspace: activeWorkspace,
    session: session ?? null,
    blocks,
    firstLoopDraft,
    deliverySummary,
  }), [activeWorkspace, blocks, deliverySummary, firstLoopDraft, session]);

  useEffect(() => {
    const toggleHandler = () => setOpen((value) => !value);
    const openHandler = (event: Event) => {
      setOpen(true);
      if ((event as CustomEvent<{ section?: string }>).detail?.section === "records") {
        setRecordsRequestedOpen(true);
      }
    };
    const shortcutHandler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "i") return;
      event.preventDefault();
      setOpen((value) => !value);
    };
    window.addEventListener("toggle-hub", toggleHandler);
    window.addEventListener("open-hub", openHandler);
    window.addEventListener("keydown", shortcutHandler);
    return () => {
      window.removeEventListener("toggle-hub", toggleHandler);
      window.removeEventListener("open-hub", openHandler);
      window.removeEventListener("keydown", shortcutHandler);
    };
  }, []);

  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open]);

  useEffect(() => {
    let cancelled = false;

    setProjectPath(activeWorkspace?.path ?? null);
    if (!open || !activeId) return;

    getProjectRuntimeStatus(activeId)
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

  if (!open) return null;

  return (
    <>
      <aside
        data-testid="project-archive-panel"
        aria-label="项目档案"
        className="forge-inspector fixed right-0 top-0 z-50 flex h-full flex-col overflow-hidden animate-[slide-in-right_0.25s_ease-out]"
      >
        <div className="forge-inspector-header">
          <span className="text-xs font-semibold text-foreground">项目档案</span>
          <button
            type="button"
            aria-label="关闭项目档案"
            onClick={() => setOpen(false)}
            className="forge-icon-button"
            title="关闭项目档案"
          >
            <X className="size-4" />
          </button>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div data-testid="project-archive-body" className="forge-inspector-body">
            <ProjectOverviewCard overview={projectOverview} />

            <CurrentTaskCard workflow={workflow} />

            {firstLoopDraft && <FirstLoopCard draft={firstLoopDraft} />}

            {activeContextItems.length > 0 && <ActiveContextSection items={activeContextItems} />}

            <ArchiveDisclosure
              testId="archive-disclosure-records"
              title="项目记录"
              meta="记录与建议"
              defaultOpen={recordsRequestedOpen || deliverySummary?.record_status === "pending"}
            >
              <WikiSections sessionId={activeId} projectPath={projectPath} />
            </ArchiveDisclosure>

            <ArchiveDisclosure
              testId="archive-disclosure-files"
              title="资料"
              meta={contextFiles.length > 0 ? `${contextFiles.length} 个文件` : "未添加"}
            >
              <ContextFilesSection files={contextFiles} />
            </ArchiveDisclosure>

            <ProductLayerHeader title="交付" meta="最近状态" />

            {activeId ? (
              <ProjectStatusCard sessionId={activeId} />
            ) : (
              <div className="forge-empty">
                选择一个任务后查看预览和检查点。
              </div>
            )}
          </div>
        </ScrollArea>
      </aside>
    </>
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
          <Icon className="size-3.5 shrink-0 text-muted-foreground/75" />
          <span className="truncate text-[11px] font-medium text-muted-foreground">{title}</span>
        </span>
        {meta && <span className="shrink-0 text-[10px] text-muted-foreground/65">{meta}</span>}
      </button>
      {open && <div className="mt-2 space-y-3">{children}</div>}
    </section>
  );
}

function ProductLayerHeader({ title, meta }: { title: string; meta?: string | null }) {
  return (
    <div className="flex items-center justify-between border-t border-border pt-3 first:border-t-0 first:pt-0">
      <h3 className="text-[11px] font-semibold text-foreground">{title}</h3>
      {meta && <span className="text-[10px] text-muted-foreground/70">{meta}</span>}
    </div>
  );
}

function ContextFilesSection({ files }: { files: ContextFile[] }) {
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
        <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] gap-2 border-b border-border px-3 py-2 text-[10px] uppercase tracking-wider text-muted-foreground/70">
          <span>文件名</span>
          <span>类型</span>
          <span>解析状态</span>
          <span className="text-right">参考</span>
        </div>

        {files.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 px-3 py-8 text-center">
            <FileText className="size-5 text-muted-foreground/60" />
            <div className="text-xs text-muted-foreground">还没有添加资料</div>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {files.map((file) => (
              <ContextFileRow key={file.id} file={file} />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function ContextFileRow({ file }: { file: ContextFile }) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-xs">
      <div className="min-w-0">
        <div className="truncate text-foreground">{file.name}</div>
      </div>
      <span className="truncate font-mono text-[10px] text-muted-foreground">{file.type}</span>
      <span className={cn("truncate text-[10px]", statusClass(file.status))}>
        {statusLabel(file.status)}
      </span>
      <span className="text-right text-[10px] text-muted-foreground">
        {file.inContext ? "已加入" : "未加入"}
      </span>
    </div>
  );
}

function statusLabel(status: ParseStatus) {
  switch (status) {
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
    case "pending":
      return "text-primary";
    case "parsed":
      return "text-emerald-400";
    case "failed":
      return "text-destructive";
  }
}
