import { useEffect, useState } from "react";
import { FolderOpen, PanelRightOpen, Search, Sparkles } from "lucide-react";
import { useStore } from "@/store";
import { Sidebar, type SidebarPanel } from "./Sidebar";
import { SessionView } from "@/components/session/SessionView";
import { HubPanel } from "./HubPanel";
import { CapabilityDrawer } from "./CapabilityDrawer";
import { useOutputStream } from "@/hooks/useOutputStream";
import { CommandPalette } from "@/components/CommandPalette";
import { WorkflowStatusPill } from "@/components/workflow/WorkflowStatusPill";
import type { CapabilityTab } from "@/components/settings/CapabilityManager";
import { getRememberedWorkingDir } from "@/lib/tauri";
import { getProviderModelLabel, getProjectDisplay, getSessionMeta, getSessionStatus, getSessionTitle } from "@/lib/session-display";
import { formatContextWindow, getModelContextWindow } from "@/lib/providers";
import forgeMark from "@/assets/forge-mark.svg";

export function AppShell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [activeSidebarPanel, setActiveSidebarPanel] = useState<SidebarPanel | null>(null);
  const [workingDir, setWorkingDir] = useState<string | null>(() => getRememberedWorkingDir());
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const workflow = useStore((s) => activeSessionId ? s.workflowBySession.get(activeSessionId) ?? null : null);
  const selectedMemoryCount = useStore((s) =>
    activeSessionId ? s.selectedContextBySession.get(activeSessionId)?.filter((item) => item.injected).length ?? 0 : 0,
  );
  const selectedWikiPageCount = useStore((s) =>
    activeSessionId ? s.forgeWikiContextBySession.get(activeSessionId)?.filter((item) => item.injected).length ?? 0 : 0,
  );
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const status = getSessionStatus(activeSession);
  const project = getProjectDisplay(workingDir);
  const contextWindow = activeSession?.contextWindowTokens ?? getModelContextWindow(activeSession?.model || selectedModel);
  const contextWindowLabel = formatContextWindow(contextWindow);
  const activeContextCount = selectedMemoryCount + selectedWikiPageCount;
  useOutputStream(activeSessionId);
  const capabilityTab: CapabilityTab = activeSidebarPanel === "automation" ? "hooks" : "skills";

  useEffect(() => {
    const timer = window.setInterval(() => {
      setWorkingDir(getRememberedWorkingDir());
    }, 1200);
    return () => window.clearInterval(timer);
  }, []);

  const toggleSidebarPanel = (panel: SidebarPanel) => {
    setActiveSidebarPanel((current) => (current === panel ? null : panel));
  };

  return (
    <div className="h-screen grid bg-background" style={{ gridTemplateColumns: "240px minmax(0, 1fr)" }}>
      <Sidebar
        activePanel={activeSidebarPanel}
        onOpenPanel={toggleSidebarPanel}
        onOpenSearch={() => {
          setActiveSidebarPanel(null);
          setSearchOpen(true);
        }}
      />
      <main className="flex flex-col h-full min-w-0 overflow-hidden border-r border-border">
        <div className="flex h-12 flex-shrink-0 items-center justify-between gap-4 border-b border-border px-4">
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex min-w-0 flex-col">
              <div className="flex min-w-0 items-center gap-2">
                <span className="truncate text-sm font-medium text-foreground">
                  {getSessionTitle(activeSession)}
                </span>
                <span
                  className="inline-flex shrink-0 items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[10px]"
                  style={{ color: status.color }}
                >
                  <span className="size-1.5 rounded-full" style={{ background: status.color }} />
                  {status.label}
                </span>
                <WorkflowStatusPill
                  workflow={workflow}
                  activeContextCount={activeContextCount}
                  onOpenContext={() => window.dispatchEvent(new Event("open-hub"))}
                />
              </div>
              <div className="mt-0.5 flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground/75">
                <FolderOpen className="size-3 shrink-0" />
                <span className="truncate">{project.name}</span>
                <span className="shrink-0">·</span>
                <span className="truncate">{getSessionMeta(activeSession)}</span>
              </div>
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={() => setSearchOpen(true)}
              className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border px-2 text-[11px] text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
            >
              <Search className="size-3.5" />
              搜索
            </button>
            <div className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border px-2 text-[11px] text-muted-foreground">
              <Sparkles className="size-3.5 text-primary" />
              {getProviderModelLabel(activeSession?.agentType || selectedProvider, activeSession?.model || selectedModel)}
              {contextWindowLabel && (
                <span className="text-muted-foreground/75">· 上下文 {contextWindowLabel}</span>
              )}
            </div>
            <button
              onClick={() => window.dispatchEvent(new Event("toggle-hub"))}
              className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border px-2 text-[11px] text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
              title="打开上下文"
            >
              <PanelRightOpen className="size-4" />
              上下文
            </button>
          </div>
        </div>

        {activeSessionId && sessions.has(activeSessionId) ? (
          <SessionView sessionId={activeSessionId} />
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-4 px-6 text-center">
            <img src={forgeMark} alt="" className="size-12 rounded-lg" />
            <div>
              <p className="text-sm text-muted-foreground">创建一个任务开始</p>
              <p className="mt-1 text-xs text-muted-foreground/70">
                当前项目：{project.path}
              </p>
            </div>
          </div>
        )}
      </main>
      <CapabilityDrawer
        open={activeSidebarPanel !== null}
        initialTab={capabilityTab}
        title={activeSidebarPanel === "automation" ? "自动化" : "插件"}
        onClose={() => setActiveSidebarPanel(null)}
      />
      <HubPanel />
      <CommandPalette open={searchOpen} onOpenChange={setSearchOpen} />
    </div>
  );
}
