import { useState, useEffect } from "react";
import type { ReactNode } from "react";
import { Blocks, Clock3, FolderOpen, Search, SquarePen, Trash2 } from "lucide-react";
import { useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { Input } from "@/components/ui/input";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { cn } from "@/lib/utils";
import { getDefaultWorkingDir, getRememberedWorkingDir, rememberWorkingDir } from "@/lib/tauri";
import { getSessionMeta, getSessionStatus, getSessionTitle } from "@/lib/session-display";
import forgeMark from "@/assets/forge-mark.svg";

export type SidebarPanel = "plugins" | "automation";

interface SidebarProps {
  activePanel: SidebarPanel | null;
  onOpenPanel: (panel: SidebarPanel) => void;
  onOpenSearch: () => void;
}

export function Sidebar({ activePanel, onOpenPanel, onOpenSearch }: SidebarProps) {
  const [workingDir, setWorkingDir] = useState("");

  useEffect(() => {
    getDefaultWorkingDir()
      .then((defaultDir) => {
        const remembered = getRememberedWorkingDir();
        setWorkingDir(remembered && !isBroadLocalPath(remembered) ? remembered : defaultDir);
      })
      .catch(() => {
        const remembered = getRememberedWorkingDir();
        setWorkingDir(remembered && !isBroadLocalPath(remembered) ? remembered : "");
      });
  }, []);
  useEffect(() => { rememberWorkingDir(workingDir); }, [workingDir]);

  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const { create, kill } = useSession();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const newSession = async () => {
    if (isBroadLocalPath(workingDir)) {
      alert("请选择一个具体的项目文件夹，不要直接使用用户主目录。");
      return;
    }
    try { await create(workingDir, selectedProvider, selectedModel); }
    catch (e) { alert("Failed: " + String(e)); }
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

      <nav className="mb-4 flex flex-col gap-1">
        <SidebarAction icon={<SquarePen className="size-4" />} label="新对话" onClick={newSession} />
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

      {/* Working dir */}
      <div className="pb-3 pt-2">
        <div className="mb-1 px-1 text-[9px] font-medium uppercase tracking-widest text-muted-foreground/65">项目目录</div>
        <div className="relative">
          <FolderOpen className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/65" />
          <Input value={workingDir} onChange={(e) => setWorkingDir(e.target.value)}
            className="pl-7 h-7 text-[10px] rounded-lg border-0 bg-sidebar-accent/30 text-muted-foreground" />
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
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex h-9 w-full items-center gap-3 rounded-lg px-2.5 text-sm transition-colors",
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

function isBroadLocalPath(path: string): boolean {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return true;
  return /^\/Users\/[^/]+$/.test(normalized) || /^\/home\/[^/]+$/.test(normalized);
}
