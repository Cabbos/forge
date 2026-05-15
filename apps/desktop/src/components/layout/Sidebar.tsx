import { useState } from "react";
import type { ReactNode } from "react";
import { Blocks, Clock3, FolderOpen, Search, SquarePen, Trash2 } from "lucide-react";
import { useActiveWorkspace, useStore, useSessionList, useWorkspaceList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { cn } from "@/lib/utils";
import { getSessionMeta, getSessionStatus, getSessionTitle } from "@/lib/session-display";
import { hasTauriRuntime, pickWorkspaceFolder } from "@/lib/tauri";
import { isBroadWorkspacePath, workspaceFromPath } from "@/lib/workspaces";
import forgeMark from "@/assets/forge-mark.svg";

export type SidebarPanel = "plugins" | "automation";

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
  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const setActiveWorkspace = useStore((s) => s.setActiveWorkspace);
  const upsertWorkspace = useStore((s) => s.upsertWorkspace);
  const sessions = useSessionList();
  const workspaces = useWorkspaceList();
  const activeWorkspace = useActiveWorkspace();
  const { create, kill } = useSession();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const newSession = async () => {
    if (!activeWorkspace) {
      alert("请先选择一个项目工作空间。");
      return;
    }
    try { await create(activeWorkspace.path, selectedProvider, selectedModel); }
    catch (e) { alert("Failed: " + String(e)); }
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
      style={{ borderRight: "1px solid var(--sidebar-border)" }}
    >
      {/* Brand */}
      <div className="flex items-center justify-between px-1 py-4">
        <img src={forgeMark} alt="" className="size-7 flex-shrink-0 rounded-md" />
        <div className="flex items-center gap-1.5">
          <span className="text-xs font-semibold text-sidebar-foreground tracking-tight">Forge</span>
          <SettingsDialog />
        </div>
      </div>

      <div className="relative mb-3 px-1">
        <div className="mb-1 text-[9px] font-medium uppercase tracking-widest text-muted-foreground/65">当前工作空间</div>
        <button
          type="button"
          onClick={toggleWorkspaceMenu}
          className="flex w-full items-center gap-2 rounded-lg border border-sidebar-border bg-sidebar-accent/30 px-2.5 py-2 text-left transition-colors hover:bg-sidebar-accent/55"
          aria-expanded={workspaceMenuOpen}
        >
          <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12px] font-medium text-sidebar-foreground">
              {activeWorkspace?.name ?? "选择一个项目开始"}
            </span>
            <span className="mt-0.5 block truncate text-[9px] text-muted-foreground/70">
              {activeWorkspace?.path ?? "新对话会在所选项目中创建"}
            </span>
          </span>
        </button>
        {workspaceMenuOpen && (
          <div
            className="absolute left-1 right-1 top-full z-30 mt-1 overflow-hidden rounded-lg border border-border shadow-xl"
            style={{ background: "var(--popover)" }}
          >
            {workspaces.length > 0 && (
              <div className="max-h-52 overflow-y-auto py-1">
                {workspaces.map((workspace) => (
                  <button
                    key={workspace.id}
                    type="button"
                    onClick={() => {
                      setActiveWorkspace(workspace.id);
                      setWorkspaceMenuOpen(false);
                    }}
                    className="flex w-full flex-col px-3 py-2 text-left text-xs hover:bg-secondary"
                  >
                    <span className="truncate text-foreground">{workspace.name}</span>
                    <span className="mt-0.5 truncate text-[10px] text-muted-foreground">{workspace.path}</span>
                  </button>
                ))}
              </div>
            )}
            <button
              type="button"
              onClick={chooseWorkspaceFolder}
              disabled={choosingWorkspace}
              className="flex w-full items-center gap-2 border-t border-border px-3 py-2 text-left text-xs text-foreground hover:bg-secondary disabled:cursor-default disabled:opacity-60"
            >
              <FolderOpen className="size-3.5" />
              {choosingWorkspace ? "正在打开..." : "选择文件夹"}
            </button>
            <button
              type="button"
              onClick={() => {
                setManualWorkspaceEntry(true);
                setWorkspacePathError(null);
              }}
              className="flex w-full items-center gap-2 border-t border-border px-3 py-2 text-left text-xs text-muted-foreground hover:bg-secondary hover:text-foreground"
            >
              <FolderOpen className="size-3.5" />
              手动输入路径
            </button>
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

      <nav className="mb-4 flex flex-col gap-1">
        <SidebarAction icon={<SquarePen className="size-4" />} label="新对话" disabled={!activeWorkspace} onClick={newSession} />
        <p className="px-2.5 pb-1 text-[9px] leading-snug text-muted-foreground/60">
          {activeWorkspace ? `新对话会创建在 ${activeWorkspace.name}` : "选择项目后才能创建新对话"}
        </p>
        <SidebarAction icon={<Search className="size-4" />} label="搜索" onClick={onOpenSearch} />
        <SidebarAction
          icon={<Blocks className="size-4" />}
          label="插件"
          active={activePanel === "plugins"}
          onClick={() => onOpenPanel("plugins")}
        />
        <SidebarAction
          icon={<Clock3 className="size-4" />}
          label="自动化"
          active={activePanel === "automation"}
          onClick={() => onOpenPanel("automation")}
        />
      </nav>

      {/* Sessions */}
      <div className="flex-1 min-h-0 flex flex-col">
        <div className="flex items-center justify-between mb-2 px-1">
          <span className="text-[9px] font-medium uppercase tracking-widest text-muted-foreground/70">任务</span>
          <span className="text-[9px] tabular-nums text-muted-foreground/65">{sessions.length}</span>
        </div>
        <div className="flex-1 overflow-y-auto space-y-0.5">
          {sessions.map((s) => {
            const isActive = s.id === activeSessionId;
            const status = getSessionStatus(s);
            return (
              <div key={s.id} onClick={() => setActiveSession(s.id)}
                className={cn("flex items-start gap-2.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all group",
                  isActive ? "bg-sidebar-accent text-sidebar-accent-foreground" : "text-muted-foreground hover:text-sidebar-foreground hover:bg-sidebar-accent/40")}>
                <span className="mt-1.5 w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: status.color }} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[11px] font-medium">{getSessionTitle(s)}</div>
                  <div className="mt-0.5 truncate text-[9px] text-muted-foreground/70">{getSessionMeta(s)}</div>
                </div>
                <Trash2 className="size-3 opacity-0 group-hover:opacity-50 hover:opacity-100 text-destructive cursor-pointer flex-shrink-0"
                  onClick={(e) => { e.stopPropagation(); kill(s.id); }} />
              </div>
            );
          })}
          {sessions.length === 0 && (
            <p className="text-[11px] text-center py-8 text-muted-foreground/60">
              还没有任务
            </p>
          )}
        </div>
      </div>

      {/* Version */}
      <div className="py-2 px-1 border-t border-sidebar-border/50">
        <p className="text-[8px] font-mono text-muted-foreground/50">v0.4 · Forge</p>
      </div>
    </aside>
  );
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
        "flex h-9 w-full items-center gap-3 rounded-lg px-2.5 text-sm transition-colors",
        disabled && "cursor-default opacity-45",
        active
          ? "bg-sidebar-accent text-sidebar-accent-foreground"
          : "text-muted-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-foreground",
      )}
    >
      <span className={cn("flex size-4 items-center justify-center", active && "text-primary")}>
        {icon}
      </span>
      <span className="truncate">{label}</span>
    </button>
  );
}
