import { useEffect, useState } from "react";
import { FilePlus2, FileText, X } from "lucide-react";
import { useStore } from "@/store";
import { ScrollArea } from "@/components/ui/scroll-area";
import { WikiSections } from "@/components/context/WikiSections";
import { ProjectStatusCard } from "./ProjectStatusCard";
import { cn } from "@/lib/utils";
import { formatContextWindow, getModelContextWindow, getProviderModelLabel } from "@/lib/providers";
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
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const session = activeId ? sessions.get(activeId) : null;
  const contextWindow = session?.contextWindowTokens ?? getModelContextWindow(session?.model);
  const contextWindowLabel = formatContextWindow(contextWindow);

  useEffect(() => {
    const handler = () => setOpen((value) => !value);
    window.addEventListener("toggle-hub", handler);
    return () => window.removeEventListener("toggle-hub", handler);
  }, []);

  useEffect(() => {
    let cancelled = false;

    setProjectPath(null);
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
  }, [activeId, open]);

  if (!open) return null;

  return (
    <>
      <div
        className="fixed inset-0 z-40 bg-black/20"
        onClick={() => setOpen(false)}
      />
      <aside
        className="fixed right-0 top-0 z-50 flex h-full w-[320px] flex-col overflow-hidden animate-[slide-in-right_0.25s_ease-out]"
        style={{
          background: "rgba(18,19,24,0.94)",
          backdropFilter: "blur(20px)",
          WebkitBackdropFilter: "blur(20px)",
          borderLeft: "1px solid rgba(255,255,255,0.12)",
        }}
      >
        <div className="flex flex-shrink-0 items-center justify-between px-4 py-3">
          <span className="text-xs font-semibold text-foreground">上下文</span>
          <button
            onClick={() => setOpen(false)}
            className="text-muted-foreground transition-colors hover:text-foreground"
            title="关闭"
          >
            <X className="size-4" />
          </button>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div className="flex flex-col gap-4 p-4">
            <WikiSections sessionId={activeId} projectPath={projectPath} />

            <ContextFilesSection files={contextFiles} />

            <section>
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-[11px] font-medium text-muted-foreground">项目状态</h3>
                <span className="text-[10px] text-muted-foreground/70">轻量</span>
              </div>
              <ProjectStatusCard sessionId={activeId} />
            </section>

            {session && (
              <div className="flex flex-col gap-1 border-t border-border pt-4">
                <div className="flex justify-between gap-3 text-xs font-mono text-muted-foreground">
                  <span className="min-w-0 truncate">{getProviderModelLabel(session.agentType, session.model)}</span>
                  <span className="shrink-0 text-primary">${session.costUsd.toFixed(2)}</span>
                </div>
                {contextWindowLabel && (
                  <div className="text-[11px] text-muted-foreground/75">
                    上下文长度：{contextWindowLabel}
                  </div>
                )}
              </div>
            )}
          </div>
        </ScrollArea>
      </aside>
    </>
  );
}

function ContextFilesSection({ files }: { files: ContextFile[] }) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">资料</h3>
        <button
          type="button"
          disabled
          className="inline-flex items-center gap-1.5 rounded-md border border-border px-2 py-1 text-[11px] text-muted-foreground transition-colors disabled:cursor-default disabled:opacity-70"
          title="后续接入文件上传与解析"
        >
          <FilePlus2 className="size-3" />
          添加文件
        </button>
      </div>

      <div className="overflow-hidden rounded-md border border-border bg-card">
        <div className="grid grid-cols-[minmax(0,1fr)_48px_64px] gap-2 border-b border-border px-3 py-2 text-[10px] uppercase tracking-wider text-muted-foreground/70">
          <span>文件</span>
          <span>类型</span>
          <span className="text-right">上下文</span>
        </div>

        {files.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 px-3 py-8 text-center">
            <FileText className="size-5 text-muted-foreground/60" />
            <div className="text-xs text-muted-foreground">还没有添加资料</div>
            <div className="max-w-[220px] text-[11px] leading-relaxed text-muted-foreground/70">
              之后可在这里接入 PPT、Word、Excel、PDF 等资料解析。
            </div>
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
    <div className="grid grid-cols-[minmax(0,1fr)_48px_64px] items-center gap-2 px-3 py-2 text-xs">
      <div className="min-w-0">
        <div className="truncate text-foreground">{file.name}</div>
        <div className={cn("mt-0.5 text-[10px]", statusClass(file.status))}>
          {statusLabel(file.status)}
        </div>
      </div>
      <span className="truncate font-mono text-[10px] text-muted-foreground">{file.type}</span>
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
