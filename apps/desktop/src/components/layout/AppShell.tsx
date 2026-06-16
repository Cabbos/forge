import { lazy, Suspense, useEffect, useState } from "react";
import { useActiveWorkspace, useStore } from "@/store";
import { Sidebar, type SidebarPanel } from "./Sidebar";
import { EmptyWorkbench } from "./EmptyWorkbench";
import { AppTitlebar } from "./AppTitlebar";
import { HubPanelHost } from "./HubPanelHost";
import { useOutputStream } from "@/hooks/useOutputStream";
import type { CapabilityTab } from "@/components/settings/CapabilityManager";
import { getProjectDisplay } from "@/lib/session-display";
import { useEmptyWorkbenchController } from "./useEmptyWorkbenchController";
import { RecoveryNoticeBanner } from "./RecoveryNoticeBanner";
import { HealthAlertBanner } from "./HealthAlertBanner";
import { NetworkStatusBanner } from "./NetworkStatusBanner";
import { StatusBar } from "@/components/StatusBar";

const LazySessionView = lazy(() => import("@/components/session/SessionView").then((module) => ({ default: module.SessionView })));
const LazyCapabilityDrawer = lazy(() => import("./CapabilityDrawer").then((module) => ({ default: module.CapabilityDrawer })));
const LazyCommandPalette = lazy(() => import("@/components/CommandPalette").then((module) => ({ default: module.CommandPalette })));

export function AppShell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [activeSidebarPanel, setActiveSidebarPanel] = useState<SidebarPanel | null>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const activeWorkspace = useActiveWorkspace();
  const { emptyWorkbenchProps, startConversation } = useEmptyWorkbenchController();
  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const project = getProjectDisplay(activeSession?.workingDir || activeWorkspace?.path);
  const visibleSessionId = activeSessionId && sessions.has(activeSessionId) ? activeSessionId : null;
  useOutputStream(activeSessionId);
  const capabilityTab: CapabilityTab = activeSidebarPanel === "automation" ? "hooks" : "skills";

  const toggleSidebarPanel = (panel: SidebarPanel) => {
    setActiveSidebarPanel((current) => (current === panel ? null : panel));
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      const key = event.key.toLowerCase();

      if (key === "k") {
        event.preventDefault();
        setActiveSidebarPanel(null);
        setSearchOpen(true);
        return;
      }

      if (key === "n") {
        if (isEditableTarget(event.target)) return;
        event.preventDefault();
        startConversation();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [startConversation]);

  return (
    <div
      data-testid="operating-surface"
      data-design-version="v3-light-workbench"
      className="forge-app-shell h-screen grid bg-background"
    >
      <Sidebar
        activePanel={activeSidebarPanel}
        onOpenPanel={toggleSidebarPanel}
        onOpenSearch={() => {
          setActiveSidebarPanel(null);
          setSearchOpen(true);
        }}
      />
      <main data-testid="main-workbench" className="forge-main-workbench flex flex-col h-full min-w-0 overflow-hidden">
        <RecoveryNoticeBanner />
        <HealthAlertBanner />
        <NetworkStatusBanner />
        <AppTitlebar
          session={activeSession}
          project={project}
          onOpenSearch={() => setSearchOpen(true)}
          onOpenHub={() => window.dispatchEvent(new Event("toggle-hub"))}
        />

        {visibleSessionId ? (
          <Suspense fallback={<div className="flex-1 bg-background" />}>
            <LazySessionView sessionId={visibleSessionId} />
          </Suspense>
        ) : (
          <EmptyWorkbench
            {...emptyWorkbenchProps}
            project={project}
          />
        )}
        <StatusBar />
      </main>
      {activeSidebarPanel !== null && (
        <Suspense fallback={null}>
          <LazyCapabilityDrawer
            open
            initialTab={capabilityTab}
            title={activeSidebarPanel === "automation" ? "自动化" : "插件"}
            onClose={() => setActiveSidebarPanel(null)}
          />
        </Suspense>
      )}
      <HubPanelHost />
      {searchOpen && (
        <Suspense fallback={null}>
          <LazyCommandPalette open={searchOpen} onOpenChange={setSearchOpen} />
        </Suspense>
      )}
    </div>
  );
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  return target.isContentEditable || tagName === "input" || tagName === "textarea" || tagName === "select";
}
