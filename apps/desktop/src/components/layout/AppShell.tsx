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
import { createProjectCheckpoint, pickWorkspaceFolder } from "@/lib/tauri";
import { isBroadWorkspacePath, workspaceFromPath } from "@/lib/workspaces";

const EMPTY_START_HINTS = [
  "我想做一个记录喝水次数的小工具",
  "我想做一个客户跟进小工具",
  "检查这个项目能不能运行",
];

type EmptyStartMode = "new-tool" | "existing-project";

export function AppShell() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [activeSidebarPanel, setActiveSidebarPanel] = useState<SidebarPanel | null>(null);
  const [emptyPrompt, setEmptyPrompt] = useState("");
  const [emptyStartMode, setEmptyStartMode] = useState<EmptyStartMode | null>(null);
  const [emptyWorkspaceNotice, setEmptyWorkspaceNotice] = useState<string | null>(null);
  const [emptyPromptStarting, setEmptyPromptStarting] = useState(false);
  const emptyPromptRef = useRef<HTMLTextAreaElement>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setFirstLoopDraft = useStore((s) => s.setFirstLoopDraft);
  const addUserMessage = useStore((s) => s.addUserMessage);
  const upsertWorkspace = useStore((s) => s.upsertWorkspace);
  const { create, send } = useSession();
  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const status = getSessionStatus(activeSession);
  const hasPendingOutput = activeSession?.blocks.some((block) => block.event_type === "pending") ?? false;
  const titlebarStatus = hasPendingOutput ? { label: "响应中", color: "#B88A56" } : status;
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

  const emptyComposerPlaceholder = emptyStartMode === "existing-project"
    ? "描述当前项目里要改的地方，Forge 会保持在当前项目内处理"
    : "描述你想做的小工具，例如：记录喝水次数、客户跟进、番茄钟";
  const emptyComposerContext = emptyStartMode === "existing-project"
    ? "打开已有项目 · Enter 发送 · Shift+Enter 换行"
    : "做个新工具 · Enter 发送 · Shift+Enter 换行";

  const focusEmptyComposer = useCallback(() => {
    requestAnimationFrame(() => {
      emptyPromptRef.current?.focus();
    });
  }, []);

  const activateWorkspaceFromPath = useCallback((path: string): boolean => {
    if (!path) {
      setEmptyWorkspaceNotice("先选择一个保存位置或已有项目文件夹。");
      return false;
    }
    if (isBroadWorkspacePath(path)) {
      setEmptyWorkspaceNotice("请选择更具体的文件夹，不要直接使用用户主目录。");
      return false;
    }
    const workspace = workspaceFromPath(path);
    if (!workspace) {
      setEmptyWorkspaceNotice("这个路径暂时不能作为本地工作空间。");
      return false;
    }
    upsertWorkspace(workspace);
    setEmptyWorkspaceNotice(null);
    return true;
  }, [upsertWorkspace]);

  const chooseWorkspaceForEmptyState = useCallback(async (): Promise<boolean> => {
    setEmptyWorkspaceNotice(null);
    try {
      const selectedPath = await pickWorkspaceFolder();
      if (!selectedPath) {
        setEmptyWorkspaceNotice("先选择保存位置或已有项目文件夹，再开始对话。");
        return false;
      }
      return activateWorkspaceFromPath(selectedPath);
    } catch (error) {
      console.error("Failed to choose workspace from empty state:", error);
      setEmptyWorkspaceNotice("没有打开文件夹选择器，请从左侧选择项目。");
      return false;
    }
  }, [activateWorkspaceFromPath]);

  const selectNewToolEntry = useCallback(async () => {
    setEmptyStartMode("new-tool");
    if (!activeWorkspace) {
      const selected = await chooseWorkspaceForEmptyState();
      if (!selected) return;
    }
    focusEmptyComposer();
  }, [activeWorkspace, chooseWorkspaceForEmptyState, focusEmptyComposer]);

  const selectExistingProjectEntry = useCallback(async () => {
    setEmptyStartMode("existing-project");
    if (!activeWorkspace) {
      const selected = await chooseWorkspaceForEmptyState();
      if (!selected) return;
    }
    focusEmptyComposer();
  }, [activeWorkspace, chooseWorkspaceForEmptyState, focusEmptyComposer]);

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
      await createProjectCheckpoint(sessionId, activeWorkspace.path).catch(() => {});
      addUserMessage(sessionId, text);
      await send(sessionId, buildFirstLoopAgentPrompt(text, { workingDir: activeWorkspace.path }), []);
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
    setEmptyStartMode(hint.includes("检查") || hint.includes("优化") ? "existing-project" : "new-tool");
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
              <div data-testid="empty-middle-hints" className="forge-empty-hints">
                <div className="forge-empty-hints-inner">
                  <div className="forge-empty-entry-grid" aria-label="开始方式">
                    <button
                      type="button"
                      data-testid="empty-entry-new-tool"
                      data-active={emptyStartMode === "new-tool"}
                      onClick={selectNewToolEntry}
                      className="forge-empty-entry-card"
                    >
                      <span className="forge-empty-entry-icon">
                        <SquarePen className="size-4" />
                      </span>
                      <span className="forge-empty-entry-copy">
                        <span className="forge-empty-entry-title">做个新工具</span>
                        <span className="forge-empty-entry-desc">
                          从一句想法开始，先做可预览的本地网页第一版。
                        </span>
                      </span>
                    </button>
                    <button
                      type="button"
                      data-testid="empty-entry-existing-project"
                      data-active={emptyStartMode === "existing-project"}
                      onClick={selectExistingProjectEntry}
                      className="forge-empty-entry-card"
                    >
                      <span className="forge-empty-entry-icon">
                        <FolderOpen className="size-4" />
                      </span>
                      <span className="forge-empty-entry-copy">
                        <span className="forge-empty-entry-title">打开已有项目</span>
                        <span className="forge-empty-entry-desc">
                          继续修改、检查、预览，所有动作绑定当前文件夹。
                        </span>
                      </span>
                    </button>
                  </div>
                  {activeWorkspace ? (
                    <>
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
                    </>
                  ) : (
                    <p data-testid="empty-workspace-notice" className="forge-empty-workspace-notice">
                      {emptyWorkspaceNotice ?? "选择保存位置或已有项目后，就可以开始对话。"}
                    </p>
                  )}
                </div>
              </div>
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
                      placeholder={emptyComposerPlaceholder}
                      rows={3}
                      className="forge-empty-composer-input"
                    />
                    <div className="forge-empty-composer-footer">
                      <span className="forge-empty-composer-context">{emptyComposerContext}</span>
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
