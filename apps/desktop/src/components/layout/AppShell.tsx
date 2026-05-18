import { useCallback, useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { ArrowUp, FolderOpen, PanelRightOpen, Search, SquarePen } from "lucide-react";
import { useActiveWorkspace, useStore } from "@/store";
import { Sidebar, type SidebarPanel } from "./Sidebar";
import { SessionView } from "@/components/session/SessionView";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import { HubPanel } from "./HubPanel";
import { CapabilityDrawer } from "./CapabilityDrawer";
import { useOutputStream } from "@/hooks/useOutputStream";
import { useSession } from "@/hooks/useSession";
import { CommandPalette } from "@/components/CommandPalette";
import type { CapabilityTab } from "@/components/settings/CapabilityManager";
import { getProjectDisplay, getSessionStatus, getSessionTitle } from "@/lib/session-display";
import { buildFirstLoopAgentPrompt, deriveFirstLoopDraft } from "@/lib/first-loop";
import { createProjectCheckpoint } from "@/lib/tauri";

const EMPTY_START_HINTS = [
  "做一个可以预览的小工具",
  "检查这个项目能不能运行",
  "继续优化当前页面体验",
];

export function AppShell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [activeSidebarPanel, setActiveSidebarPanel] = useState<SidebarPanel | null>(null);
  const [emptyPrompt, setEmptyPrompt] = useState("");
  const [emptyPromptStarting, setEmptyPromptStarting] = useState(false);
  const emptyPromptRef = useRef<HTMLTextAreaElement>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setFirstLoopDraft = useStore((s) => s.setFirstLoopDraft);
  const addUserMessage = useStore((s) => s.addUserMessage);
  const { create, send } = useSession();
  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const status = getSessionStatus(activeSession);
  const hasPendingOutput = activeSession?.blocks.some((block) => block.event_type === "pending") ?? false;
  const titlebarStatus = hasPendingOutput ? { label: "响应中", color: "#D4A853" } : status;
  const showSessionStatus = hasPendingOutput || activeSession?.streaming || activeSession?.status === "error";
  const titlebarStatusState = activeSession?.status === "error"
    ? "error"
    : hasPendingOutput || activeSession?.streaming
      ? "running"
      : "idle";
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

  const startConversationWithPrompt = useCallback(async () => {
    const text = emptyPrompt.trim();
    if (!activeWorkspace || !text || emptyPromptStarting) return;

    setEmptyPromptStarting(true);
    try {
      const sessionId = await create(activeWorkspace.path, selectedProvider, selectedModel);
      const firstLoopDraft = deriveFirstLoopDraft(sessionId, text);
      if (firstLoopDraft) {
        setFirstLoopDraft(sessionId, firstLoopDraft);
      }
      await createProjectCheckpoint(sessionId).catch(() => {});
      addUserMessage(sessionId, text);
      await send(sessionId, buildFirstLoopAgentPrompt(text), []);
      setEmptyPrompt("");
    } catch (error) {
      console.error("Failed to start conversation from prompt:", error);
    } finally {
      setEmptyPromptStarting(false);
    }
  }, [
    activeWorkspace,
    addUserMessage,
    create,
    emptyPrompt,
    emptyPromptStarting,
    selectedModel,
    selectedProvider,
    send,
    setFirstLoopDraft,
  ]);

  const handleEmptyPromptKeyDown = useCallback((event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    startConversationWithPrompt();
  }, [startConversationWithPrompt]);

  const useEmptyHint = useCallback((hint: string) => {
    setEmptyPrompt(hint);
    requestAnimationFrame(() => {
      emptyPromptRef.current?.focus();
      emptyPromptRef.current?.setSelectionRange(hint.length, hint.length);
    });
  }, []);

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
    <div className="forge-app-shell h-screen grid bg-background" style={{ gridTemplateColumns: "var(--forge-sidebar-width) minmax(0, 1fr)" }}>
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
          className="forge-titlebar forge-app-titlebar"
        >
          <div data-testid="titlebar-context" className="forge-titlebar-context">
            <div className="forge-titlebar-title-row">
              <span data-testid="titlebar-title" className="forge-titlebar-title">
                {getSessionTitle(activeSession)}
              </span>
              {showSessionStatus && (
                <span
                  data-testid="titlebar-status-pill"
                  data-state={titlebarStatusState}
                  className="forge-titlebar-status-pill"
                  style={{
                    color: titlebarStatus.color,
                    borderColor: `${titlebarStatus.color}38`,
                    backgroundColor: `${titlebarStatus.color}14`,
                  }}
                >
                  <span className="forge-titlebar-status-dot" style={{ background: titlebarStatus.color }} />
                  {titlebarStatus.label}
                </span>
              )}
            </div>
            <div
              data-testid="titlebar-project-boundary"
              aria-label="当前项目边界"
              className="forge-titlebar-project"
              title={project.path}
            >
              <FolderOpen className="forge-titlebar-project-icon" />
              <span className="forge-titlebar-project-label">当前项目</span>
              <span className="forge-titlebar-project-name">{project.name}</span>
            </div>
          </div>

          <div data-testid="titlebar-actions" className="forge-titlebar-actions">
            <button
              onClick={() => setSearchOpen(true)}
              aria-label="搜索"
              title="搜索"
              className="forge-titlebar-button"
            >
              <Search className="size-3.5" />
            </button>
            <button
              onClick={() => window.dispatchEvent(new Event("toggle-hub"))}
              aria-label="打开项目档案"
              title="打开项目档案"
              className="forge-titlebar-button"
            >
              <PanelRightOpen className="size-4" />
            </button>
          </div>
        </div>

        {activeSessionId && sessions.has(activeSessionId) ? (
          <SessionView sessionId={activeSessionId} />
        ) : (
          <div className={activeWorkspace ? "forge-empty-shell forge-empty-shell-codex" : "forge-empty-shell forge-empty-shell-centered"}>
            <div data-testid="empty-workbench" className="forge-empty-workbench">
              {activeWorkspace ? (
                <div data-testid="empty-middle-hints" className="forge-empty-hints">
                  <div className="forge-empty-hints-inner">
                    <p className="forge-empty-hints-title">可以这样开始</p>
                    <div className="forge-empty-hint-list">
                      {EMPTY_START_HINTS.map((hint) => (
                        <button
                          key={hint}
                          type="button"
                          onClick={() => useEmptyHint(hint)}
                          className="forge-empty-hint"
                        >
                          {hint}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              ) : (
                <>
                  <p className="forge-empty-title">选择一个项目开始</p>
                  <p className="forge-empty-copy">
                    先选择一个具体项目，Forge 会把对话和交付状态放在同一个工作空间里。
                  </p>
                </>
              )}
            </div>
            {activeWorkspace && (
              <div className="forge-empty-composer-frame">
                <div className="forge-conversation-lane">
                  <div className="forge-empty-context-row">
                    <div data-testid="empty-workbench-project" className="forge-empty-project">
                      <FolderOpen className="forge-empty-project-icon" />
                      <span className="forge-empty-project-name">{project.name}</span>
                    </div>
                    <button
                      type="button"
                      data-testid="empty-workbench-action"
                      onClick={startConversation}
                      className="forge-empty-action"
                    >
                      <SquarePen className="size-3.5" />
                      开始新对话
                    </button>
                  </div>
                  <div data-testid="empty-start-composer" className="forge-empty-composer">
                    <textarea
                      ref={emptyPromptRef}
                      value={emptyPrompt}
                      onChange={(event) => setEmptyPrompt(event.target.value)}
                      onKeyDown={handleEmptyPromptKeyDown}
                      placeholder="描述你想做的小工具或要改的地方"
                      rows={3}
                      className="forge-empty-composer-input"
                    />
                    <div className="forge-empty-composer-footer">
                      <span className="forge-empty-composer-context">Enter 发送 · Shift+Enter 换行</span>
                      <button
                        type="button"
                        data-testid="empty-start-send"
                        aria-label="发送并开始"
                        onClick={startConversationWithPrompt}
                        disabled={!emptyPrompt.trim() || emptyPromptStarting}
                        data-ready={emptyPrompt.trim() ? "true" : "false"}
                        className="forge-empty-composer-send"
                      >
                        <ArrowUp className="size-4" />
                      </button>
                    </div>
                  </div>
                  <StartReadinessCard variant="setup-strip" />
                </div>
              </div>
            )}
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
