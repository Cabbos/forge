import { useCallback, useEffect, useState } from "react";
import { FolderOpen, PanelRightOpen, Search } from "lucide-react";
import { useActiveWorkspace, useStore } from "@/store";
import { Sidebar, type SidebarPanel } from "./Sidebar";
import { SessionView } from "@/components/session/SessionView";
import { HubPanel } from "./HubPanel";
import { CapabilityDrawer } from "./CapabilityDrawer";
import { useOutputStream } from "@/hooks/useOutputStream";
import { useSession } from "@/hooks/useSession";
import { CommandPalette } from "@/components/CommandPalette";
import type { CapabilityTab } from "@/components/settings/CapabilityManager";
import { getProjectDisplay, getSessionStatus, getSessionTitle } from "@/lib/session-display";

export function AppShell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [activeSidebarPanel, setActiveSidebarPanel] = useState<SidebarPanel | null>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const { create } = useSession();
  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const status = getSessionStatus(activeSession);
  const showSessionStatus = activeSession?.streaming || activeSession?.status === "error";
  const project = getProjectDisplay(activeSession?.workingDir || activeWorkspace?.path);
  useOutputStream(activeSessionId);
  const capabilityTab: CapabilityTab = activeSidebarPanel === "automation" ? "hooks" : "skills";

  const toggleSidebarPanel = (panel: SidebarPanel) => {
    setActiveSidebarPanel((current) => (current === panel ? null : panel));
  };

  const startConversation = useCallback(() => {
    if (!activeWorkspace) return;
    create(activeWorkspace.path, selectedProvider, selectedModel).catch((error) => {
      console.error("Failed to create session:", error);
    });
  }, [activeWorkspace, create, selectedModel, selectedProvider]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "n") return;
      if (isEditableTarget(event.target)) return;

      event.preventDefault();
      startConversation();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [startConversation]);

  return (
    <div className="h-screen grid bg-background" style={{ gridTemplateColumns: "220px minmax(0, 1fr)" }}>
      <Sidebar
        activePanel={activeSidebarPanel}
        onOpenPanel={toggleSidebarPanel}
        onOpenSearch={() => {
          setActiveSidebarPanel(null);
          setSearchOpen(true);
        }}
      />
      <main className="flex flex-col h-full min-w-0 overflow-hidden border-r border-border">
        <div
          data-testid="app-titlebar"
          data-tauri-drag-region="true"
          className="forge-titlebar flex flex-shrink-0 items-center justify-between gap-4 border-b border-border px-4"
        >
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex min-w-0 flex-col">
              <div className="flex min-w-0 items-center gap-2">
                <span className="truncate text-sm font-medium text-foreground">
                  {getSessionTitle(activeSession)}
                </span>
                {showSessionStatus && (
                  <span
                    className="inline-flex shrink-0 items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[10px]"
                    style={{ color: status.color }}
                  >
                    <span className="size-1.5 rounded-full" style={{ background: status.color }} />
                    {status.label}
                  </span>
                )}
              </div>
              <div
                aria-label="当前项目边界"
                className="mt-0.5 flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground/75"
                title={project.path}
              >
                <FolderOpen className="size-3 shrink-0" />
                <span className="shrink-0 text-muted-foreground/60">当前项目</span>
                <span className="truncate text-foreground/80">{project.name}</span>
              </div>
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={() => setSearchOpen(true)}
              aria-label="搜索"
              title="搜索"
              className="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
            >
              <Search className="size-3.5" />
            </button>
            <button
              onClick={() => window.dispatchEvent(new Event("toggle-hub"))}
              aria-label="打开项目档案"
              title="打开项目档案"
              className="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
            >
              <PanelRightOpen className="size-4" />
            </button>
          </div>
        </div>

        {activeSessionId && sessions.has(activeSessionId) ? (
          <SessionView sessionId={activeSessionId} />
        ) : (
          <div className="flex h-full items-center justify-center px-6 text-center">
            <div data-testid="empty-workbench" className="forge-empty-workbench">
              <p className="text-sm font-medium text-foreground">
                {activeWorkspace ? "准备开始" : "选择一个项目开始"}
              </p>
              <p className="mt-1.5 max-w-[360px] text-xs leading-relaxed text-muted-foreground/75">
                {activeWorkspace
                  ? "描述你想做什么，Forge 会在当前项目里继续。"
                  : "先选择一个具体项目，Forge 会把对话和交付状态绑定到当前项目。"}
              </p>
              {activeWorkspace && (
                <button
                  type="button"
                  onClick={startConversation}
                  className="forge-action mt-4 justify-center"
                >
                  开始新对话
                </button>
              )}
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

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  return target.isContentEditable || tagName === "input" || tagName === "textarea" || tagName === "select";
}
