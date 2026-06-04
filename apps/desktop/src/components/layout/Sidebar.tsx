import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { useActiveWorkspace, useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
import {
  createSessionNotice,
  SidebarNoticeBanner,
  SidebarPrimaryNav,
  SidebarUtilityNav,
  type SidebarNotice,
  type SidebarPanel,
} from "./SidebarActions";
import { SidebarSessionHistory } from "./SidebarSessionHistory";
import { SidebarWorkspaceMenu } from "./SidebarWorkspaceMenu";
import forgeMark from "@/assets/forge-mark.svg";

export type { SidebarPanel } from "./SidebarActions";

interface SidebarProps {
  activePanel: SidebarPanel | null;
  onOpenPanel: (panel: SidebarPanel) => void;
  onOpenSearch: () => void;
}

const LazySettingsDialog = lazy(() => import("@/components/settings/SettingsDialog").then((module) => ({ default: module.SettingsDialog })));

export function Sidebar({ activePanel, onOpenPanel, onOpenSearch }: SidebarProps) {
  const [sidebarNotice, setSidebarNotice] = useState<SidebarNotice | null>(null);
  const [settingsDialogMounted, setSettingsDialogMounted] = useState(false);
  const [settingsDialogOpen, setSettingsDialogOpen] = useState(false);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const activeWorkspace = useActiveWorkspace();
  const { create, deleteConversation } = useSession();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const openSettingsDialog = useCallback(() => {
    setSettingsDialogMounted(true);
    setSettingsDialogOpen(true);
  }, []);

  useEffect(() => {
    const openSettings = () => openSettingsDialog();
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        openSettingsDialog();
      }
    };

    window.addEventListener("forge:open-settings", openSettings);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("forge:open-settings", openSettings);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [openSettingsDialog]);

  const newSession = async () => {
    if (!activeWorkspace) {
      setSidebarNotice({ message: "先选择一个具体项目，再开始新对话。" });
      return;
    }
    setSidebarNotice(null);
    try {
      await create(activeWorkspace.path, selectedProvider, selectedModel);
    } catch (error) {
      console.error("Failed to create session:", error);
      setSidebarNotice(createSessionNotice(error));
    }
  };

  return (
    <aside data-testid="app-sidebar" className="forge-sidebar h-full w-full select-none overflow-hidden">
      <div
        aria-hidden="true"
        data-tauri-drag-region="true"
        className="forge-sidebar-window-drag-region"
      />

      {/* Brand */}
      <div data-forge-motion="sidebar-entry" className="forge-sidebar-brand">
        <img src={forgeMark} alt="" className="size-7 flex-shrink-0 rounded-md" />
        <div className="forge-sidebar-brand-copy">
          <span className="forge-sidebar-brand-title">Forge</span>
          <span className="forge-sidebar-brand-subtitle">Local workbench</span>
        </div>
      </div>

      <SidebarWorkspaceMenu onWorkspaceActivated={() => setSidebarNotice(null)} />

      <SidebarPrimaryNav activeWorkspace={activeWorkspace} onNewSession={newSession} onOpenSearch={onOpenSearch} />

      {sidebarNotice && <SidebarNoticeBanner notice={sidebarNotice} />}

      <SidebarSessionHistory
        activeSessionId={activeSessionId}
        onDeleteSession={deleteConversation}
        onSelectSession={setActiveSession}
        sessions={sessions}
      />

      <SidebarUtilityNav
        activePanel={activePanel}
        onOpenPanel={onOpenPanel}
        onOpenSettings={openSettingsDialog}
      />
      {settingsDialogMounted && (
        <Suspense fallback={null}>
          <LazySettingsDialog open={settingsDialogOpen} onOpenChange={setSettingsDialogOpen} hideTrigger />
        </Suspense>
      )}
    </aside>
  );
}
