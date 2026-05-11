import { useState, useEffect } from "react";
import { Plus, Trash2, FolderOpen } from "lucide-react";
import { useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { Input } from "@/components/ui/input";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { homeDir } from "@tauri-apps/api/path";
import { cn } from "@/lib/utils";

export function Sidebar() {
  const [expanded, setExpanded] = useState(false);
  const [workingDir, setWorkingDir] = useState("");

  useEffect(() => { homeDir().then(setWorkingDir).catch(() => setWorkingDir("/")); }, []);

  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const { create, kill } = useSession();

  const newSession = async () => {
    try { await create(workingDir); }
    catch (e) { alert("Failed: " + String(e)); }
  };

  return (
    <aside
      className={cn(
        "h-full flex flex-col select-none transition-all duration-200 ease-out overflow-hidden bg-sidebar",
        expanded ? "w-[220px] px-3" : "w-[48px] px-2"
      )}
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      style={{ borderRight: "1px solid #161616" }}
    >
      {/* Brand */}
      <div className={cn("flex items-center py-4", expanded ? "justify-between px-1" : "justify-center")}>
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" className="flex-shrink-0">
          <path d="M12 3C8 3 4 7 3 11C2 13 2 16 4 17.5C5 18.5 7 18 8 17C9 16 10 14.5 12 14.5C14 14.5 15 16 16 17C17 18 19 18.5 20 17.5C22 16 22 13 21 11C20 7 16 3 12 3Z" fill="#4B9CD3" opacity="0.9" />
        </svg>
        {expanded && (
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-semibold text-sidebar-foreground tracking-tight">Deep Agent</span>
            <SettingsDialog />
          </div>
        )}
      </div>

      {/* New session */}
      {expanded ? (
        <button onClick={newSession}
          className="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium mb-3 transition-colors border border-border bg-secondary text-secondary-foreground hover:bg-secondary/80">
          <Plus className="size-3.5 text-primary" /> New Session
        </button>
      ) : (
        <button onClick={newSession} className="flex justify-center py-2 mb-2">
          <Plus className="size-4 text-primary hover:text-primary/80 transition-colors" />
        </button>
      )}

      {/* Sessions */}
      <div className="flex-1 min-h-0 flex flex-col">
        {expanded && (
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[9px] font-medium uppercase tracking-widest text-muted-foreground/40">Sessions</span>
            <span className="text-[9px] tabular-nums text-muted-foreground/30">{sessions.length}</span>
          </div>
        )}
        <div className={cn("flex-1 overflow-y-auto space-y-0.5", !expanded && "flex flex-col items-center gap-2 py-2")}>
          {sessions.map((s) => {
            const isActive = s.id === activeSessionId;
            return expanded ? (
              <div key={s.id} onClick={() => setActiveSession(s.id)}
                className={cn("flex items-center gap-2.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all group",
                  isActive ? "bg-sidebar-accent text-sidebar-accent-foreground" : "text-muted-foreground hover:text-sidebar-foreground hover:bg-sidebar-accent/40")}>
                <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: s.status === "running" ? "#D4A853" : "#333" }} />
                <span className="text-[11px] truncate flex-1">{s.agentType || "deepseek"}</span>
                <span className="text-[9px] font-mono text-muted-foreground/30">{s.id.slice(0,6)}</span>
                <Trash2 className="size-3 opacity-0 group-hover:opacity-50 hover:opacity-100 text-destructive cursor-pointer flex-shrink-0"
                  onClick={(e) => { e.stopPropagation(); kill(s.id); }} />
              </div>
            ) : (
              <div key={s.id} onClick={() => setActiveSession(s.id)} title={s.id}
                className={cn("cursor-pointer rounded-full transition-all",
                  isActive ? "ring-2 ring-primary ring-offset-2 ring-offset-[#0a0a0a]" : "opacity-40 hover:opacity-70")}>
                <span className="block w-2.5 h-2.5 rounded-full" style={{ background: s.status === "running" ? "#D4A853" : "#444" }} />
              </div>
            );
          })}
          {sessions.length === 0 && (
            <p className={cn("text-muted-foreground/20", expanded ? "text-[11px] text-center py-8" : "text-[8px]")}>
              {expanded ? "No sessions yet" : "none"}
            </p>
          )}
        </div>
      </div>

      {/* Working dir */}
      {expanded && (
        <div className="pb-3 pt-2">
          <div className="relative">
            <FolderOpen className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/30" />
            <Input value={workingDir} onChange={(e) => setWorkingDir(e.target.value)}
              className="pl-7 h-7 text-[10px] rounded-lg border-0 bg-sidebar-accent/30 text-muted-foreground" />
          </div>
        </div>
      )}

      {/* Version */}
      <div className={cn("py-2", expanded ? "px-1 border-t border-sidebar-border/50" : "flex justify-center")}>
        <p className="text-[8px] font-mono text-muted-foreground/20">{expanded ? "v0.4 · DeepSeek" : "v0.4"}</p>
      </div>
    </aside>
  );
}
