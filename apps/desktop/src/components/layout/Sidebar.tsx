import { useRef, useState } from "react";
import type { ReactNode } from "react";
import { AlertCircle, Blocks, Clock3, FolderOpen, Search, SquarePen, Trash2 } from "lucide-react";
import { useActiveWorkspace, useStore, useSessionList, useWorkspaceList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { cn } from "@/lib/utils";
import { getSessionTitle } from "@/lib/session-display";
import { hasTauriRuntime, pickWorkspaceFolder } from "@/lib/tauri";
import { isBroadWorkspacePath, workspaceFromPath } from "@/lib/workspaces";
import type { SessionState } from "@/lib/protocol";
import forgeMark from "@/assets/forge-mark.svg";

export type SidebarPanel = "plugins" | "automation";
type SidebarNotice = { message: string; action?: "settings" };

interface SidebarProps {
  activePanel: SidebarPanel | null;
  onOpenPanel: (panel: SidebarPanel) => void;
  onOpenSearch: () => void;
}

export function Sidebar({ activePanel, onOpenPanel, onOpenSearch }: SidebarProps) {
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [manualWorkspaceEntry, setManualWorkspaceEntry] = useState(false);
  const [workspacePathDraft, setWorkspacePathDraft] = useState("");
  const [workspacePathError, setWorkspacePathError] = useState<string | null>(null);
  const [choosingWorkspace, setChoosingWorkspace] = useState(false);
  const [sidebarNotice, setSidebarNotice] = useState<SidebarNotice | null>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const setActiveWorkspace = useStore((s) => s.setActiveWorkspace);
  const upsertWorkspace = useStore((s) => s.upsertWorkspace);
  const removeWorkspace = useStore((s) => s.removeWorkspace);
  const sessions = useSessionList();
  const groupedSessions = groupSessionsByRecency(sessions);
  const workspaces = useWorkspaceList();
  const activeWorkspace = useActiveWorkspace();
  const sessionRowRefs = useRef(new Map<string, HTMLDivElement>());
  const { create, deleteConversation } = useSession();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const focusSessionAt = (index: number) => {
    const nextSession = sessions[index];
    if (!nextSession) return;
    sessionRowRefs.current.get(nextSession.id)?.focus();
  };

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

  const toggleWorkspaceMenu = () => {
    setWorkspaceMenuOpen((open) => {
      if (open) {
        setManualWorkspaceEntry(false);
        setWorkspacePathError(null);
      }
      return !open;
    });
  };

  const activateWorkspacePath = (path: string): boolean => {
    if (!path) {
      setWorkspacePathError("请输入一个项目文件夹路径。");
      return false;
    }
    if (isBroadWorkspacePath(path)) {
      setWorkspacePathError("请选择具体项目文件夹，不要直接使用用户主目录。");
      return false;
    }
    const workspace = workspaceFromPath(path);
    if (!workspace) {
      setWorkspacePathError("这个路径暂时不能作为工作空间。");
      return false;
    }
    upsertWorkspace(workspace);
    setSidebarNotice(null);
    setWorkspacePathDraft("");
    setWorkspacePathError(null);
    setManualWorkspaceEntry(false);
    setWorkspaceMenuOpen(false);
    return true;
  };

  const addWorkspaceFromDraft = () => {
    activateWorkspacePath(workspacePathDraft.trim());
  };

  const chooseWorkspaceFolder = async () => {
    setWorkspacePathError(null);
    setChoosingWorkspace(true);
    try {
      const selectedPath = await pickWorkspaceFolder();
      if (!selectedPath) {
        if (!hasTauriRuntime()) setManualWorkspaceEntry(true);
        return;
      }
      activateWorkspacePath(selectedPath);
    } catch (error) {
      console.error("Failed to choose workspace folder:", error);
      setWorkspacePathError("没有打开文件夹选择器，请使用手动输入。");
      setManualWorkspaceEntry(true);
    } finally {
      setChoosingWorkspace(false);
    }
  };

  return (
    <aside
      className="h-full w-full flex flex-col select-none overflow-hidden bg-sidebar px-3"
      style={{ borderRight: "1px solid var(--forge-border-subtle)" }}
    >
      {/* Brand */}
      <div className="flex items-center justify-between px-1 py-4">
        <img src={forgeMark} alt="" className="size-7 flex-shrink-0 rounded-md" />
        <span className="text-xs font-semibold text-sidebar-foreground tracking-tight">Forge</span>
      </div>

      <div className="relative mb-2 px-1">
        <button
          type="button"
          data-testid="workspace-trigger"
          onClick={toggleWorkspaceMenu}
          title={activeWorkspace?.path}
          className="flex h-8 w-full items-center gap-2 rounded-md px-2 text-left text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
          aria-controls={workspaceMenuOpen ? "workspace-menu" : undefined}
          aria-expanded={workspaceMenuOpen}
          aria-haspopup="menu"
        >
          <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate text-[12px] font-medium">
            {activeWorkspace?.name ?? "选择项目"}
          </span>
        </button>
        {workspaceMenuOpen && (
          <div
            id="workspace-menu"
            role="menu"
            aria-label="项目工作空间"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                setWorkspaceMenuOpen(false);
              }
            }}
            className="forge-floating-menu forge-sidebar-menu"
          >
            {workspaces.length > 0 && (
              <div className="max-h-52 overflow-y-auto py-1">
                {workspaces.map((workspace) => (
                  <button
                    key={workspace.id}
                    type="button"
                    role="menuitemradio"
                    aria-checked={workspace.id === activeWorkspace?.id}
                    title={workspace.path}
                    onClick={() => {
                      setActiveWorkspace(workspace.id);
                      setWorkspaceMenuOpen(false);
                    }}
                    className="forge-menu-option"
                  >
                    <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate text-foreground">{workspace.name}</span>
                  </button>
                ))}
              </div>
            )}
            <button
              type="button"
              role="menuitem"
              onClick={chooseWorkspaceFolder}
              disabled={choosingWorkspace}
              className="forge-menu-option border-t border-border text-foreground disabled:cursor-default disabled:opacity-60"
            >
              <FolderOpen className="size-3.5" />
              {choosingWorkspace ? "正在打开..." : "选择文件夹"}
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setManualWorkspaceEntry(true);
                setWorkspacePathError(null);
              }}
              className="forge-menu-option border-t border-border text-muted-foreground hover:text-foreground"
            >
              <FolderOpen className="size-3.5" />
              手动输入路径
            </button>
            {activeWorkspace && workspaces.length > 1 && (
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  removeWorkspace(activeWorkspace.id);
                  setWorkspaceMenuOpen(false);
                }}
                className="forge-menu-option border-t border-border text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              >
                <Trash2 className="size-3.5" />
                从列表移除当前项目
              </button>
            )}
            {manualWorkspaceEntry && (
              <form
                className="border-t border-border px-3 py-3"
                onSubmit={(event) => {
                  event.preventDefault();
                  addWorkspaceFromDraft();
                }}
              >
                <label htmlFor="workspace-path-input" className="block text-[10px] font-medium text-muted-foreground">
                  项目文件夹路径
                </label>
                <input
                  id="workspace-path-input"
                  autoFocus
                  value={workspacePathDraft}
                  onChange={(event) => {
                    setWorkspacePathDraft(event.target.value);
                    setWorkspacePathError(null);
                  }}
                  placeholder="/Users/you/project/app"
                  className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs text-foreground outline-none placeholder:text-muted-foreground/55 focus:border-primary"
                />
                {workspacePathError && (
                  <p className="mt-1 text-[10px] leading-snug text-destructive">{workspacePathError}</p>
                )}
                <div className="mt-2 flex items-center justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setManualWorkspaceEntry(false);
                      setWorkspacePathError(null);
                    }}
                    className="h-7 rounded-md px-2 text-[11px] text-muted-foreground hover:bg-secondary hover:text-foreground"
                  >
                    取消
                  </button>
                  <button
                    type="submit"
                    className="h-7 rounded-md bg-primary px-2.5 text-[11px] font-medium text-primary-foreground hover:opacity-90"
                  >
                    添加
                  </button>
                </div>
              </form>
            )}
          </div>
        )}
      </div>

      <nav className="mb-3 flex flex-col gap-1">
        <SidebarAction icon={<SquarePen className="size-4" />} label="新对话" disabled={!activeWorkspace} onClick={newSession} />
        <SidebarAction icon={<Search className="size-4" />} label="搜索" onClick={onOpenSearch} />
      </nav>

      {sidebarNotice && (
        <div
          role="status"
          className="mb-3 flex items-start gap-2 rounded-md border border-primary/20 bg-primary/5 px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground"
        >
          <AlertCircle className="mt-0.5 size-3.5 shrink-0 text-primary" />
          <span className="min-w-0 flex-1">{sidebarNotice.message}</span>
          {sidebarNotice.action === "settings" && (
            <button
              type="button"
              onClick={() => window.dispatchEvent(new Event("forge:open-settings"))}
              className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium text-primary transition-colors hover:bg-primary/10"
            >
              打开设置
            </button>
          )}
        </div>
      )}

      {/* Sessions */}
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="mb-1.5 flex items-center justify-between px-1">
          <span className="text-[10px] font-medium text-muted-foreground/70">对话</span>
        </div>
        <div className="flex-1 space-y-px overflow-y-auto">
          {groupedSessions.map((group) => (
            <div key={group.label} className="space-y-px">
              <div className="px-2 pb-1 pt-2 text-[10px] font-medium text-muted-foreground/55 first:pt-0">
                {group.label}
              </div>
              {group.sessions.map((s) => {
                const index = sessions.findIndex((session) => session.id === s.id);
                const isActive = s.id === activeSessionId;
                const title = getSessionTitle(s);
                return (
                  <div
                    key={s.id}
                    ref={(node) => {
                      if (node) sessionRowRefs.current.set(s.id, node);
                      else sessionRowRefs.current.delete(s.id);
                    }}
                    role="button"
                    aria-label={title}
                    data-active={isActive ? "true" : "false"}
                    tabIndex={0}
                    onClick={() => setActiveSession(s.id)}
                    onKeyDown={(event) => {
                      if (event.key === "ArrowDown") {
                        event.preventDefault();
                        focusSessionAt(Math.min(index + 1, sessions.length - 1));
                        return;
                      }
                      if (event.key === "ArrowUp") {
                        event.preventDefault();
                        focusSessionAt(Math.max(index - 1, 0));
                        return;
                      }
                      if (event.key === "Home") {
                        event.preventDefault();
                        focusSessionAt(0);
                        return;
                      }
                      if (event.key === "End") {
                        event.preventDefault();
                        focusSessionAt(sessions.length - 1);
                        return;
                      }
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        setActiveSession(s.id);
                      }
                    }}
                    className={cn(
                      "forge-sidebar-history-row group focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/55",
                      isActive
                        ? "text-sidebar-accent-foreground"
                        : "text-muted-foreground",
                    )}
                  >
                    <span className={cn("min-w-0 flex-1 truncate", isActive && "font-medium")}>{title}</span>
                    <button
                      type="button"
                      aria-label={`删除对话 ${title}`}
                      className="flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground/45 opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-destructive/45 group-hover:opacity-100"
                      onClick={(event) => {
                        event.stopPropagation();
                        deleteConversation(s.id);
                      }}
                    >
                      <Trash2 className="size-3" />
                    </button>
                  </div>
                );
              })}
            </div>
          ))}
          {sessions.length === 0 && (
            <p className="py-8 text-center text-[11px] text-muted-foreground/55">
              还没有对话
            </p>
          )}
        </div>
      </div>

      <nav
        data-testid="sidebar-utility-nav"
        className="mt-3 flex h-10 items-center gap-1 border-t border-border pt-2"
      >
        <SidebarIconAction
          icon={<Blocks className="size-4" />}
          label="插件"
          active={activePanel === "plugins"}
          onClick={() => onOpenPanel("plugins")}
        />
        <SidebarIconAction
          icon={<Clock3 className="size-4" />}
          label="自动化"
          active={activePanel === "automation"}
          onClick={() => onOpenPanel("automation")}
        />
        <SettingsDialog triggerClassName="forge-sidebar-utility-button" />
      </nav>
    </aside>
  );
}

function groupSessionsByRecency(sessions: SessionState[]) {
  const groups: Array<{ label: string; sessions: SessionState[] }> = [];
  for (const session of sessions) {
    const label = sessionRecencyLabel(session);
    const existing = groups.find((group) => group.label === label);
    if (existing) existing.sessions.push(session);
    else groups.push({ label, sessions: [session] });
  }
  return groups;
}

function sessionRecencyLabel(session: SessionState) {
  const time = session.updatedAt ?? session.createdAt ?? Date.now();
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const yesterday = today - 24 * 60 * 60 * 1000;

  if (time >= today) return "今天";
  if (time >= yesterday) return "昨天";
  return "更早";
}

function createSessionNotice(error: unknown): SidebarNotice {
  const message = error instanceof Error ? error.message : String(error);
  if (/api key|密钥/i.test(message)) {
    return {
      message: "模型服务还没有可用密钥。添加密钥后就可以开始新对话。",
      action: "settings",
    };
  }
  return { message: "新对话没有创建成功。请检查设置后重试。" };
}

function SidebarAction({
  icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-[13px] transition-colors",
        disabled && "cursor-default opacity-45",
        active
          ? "bg-secondary text-foreground"
          : "text-muted-foreground hover:bg-secondary hover:text-foreground",
      )}
    >
      <span className={cn("flex size-4 items-center justify-center", active && "text-primary")}>
        {icon}
      </span>
      <span className="truncate">{label}</span>
    </button>
  );
}

function SidebarIconAction({
  icon,
  label,
  active,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "forge-sidebar-utility-button",
        active
          ? "bg-secondary text-foreground"
          : "text-muted-foreground hover:bg-secondary hover:text-foreground",
      )}
    >
      {icon}
    </button>
  );
}
