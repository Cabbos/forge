import { useState, useEffect } from "react";
import { Bot, Terminal, X, Plus, FolderOpen } from "lucide-react";
import { useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { homeDir } from "@tauri-apps/api/path";

export function Sidebar() {
  const [workingDir, setWorkingDir] = useState("");
  useEffect(() => { homeDir().then(setWorkingDir).catch(() => setWorkingDir("/")); }, []);

  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const { create, kill } = useSession();

  const newSession = async (type: "claude" | "bash") => {
    try { await create(type, workingDir); } catch (e) { console.error(e); }
  };

  return (
    <aside className="h-full bg-sidebar flex flex-col select-none">
      <div className="px-4 py-4 flex items-center justify-between">
        <div>
          <h1 className="text-[15px] font-semibold text-sidebar-foreground tracking-tight">TUI-to-GUI</h1>
          <p className="text-[11px] text-muted-foreground/50 mt-0.5">AI Agent Desktop</p>
        </div>
        <SettingsDialog />
      </div>

      <div className="px-3 pb-3 space-y-1.5">
        <Button variant="default" size="sm" onClick={() => newSession("claude")}
          className="w-full justify-start gap-2.5 h-9 rounded-xl text-[13px] font-medium shadow-sm">
          <Bot className="size-4" />New Claude
        </Button>
        <Button variant="ghost" size="sm" onClick={() => newSession("bash")}
          className="w-full justify-start gap-2.5 h-9 rounded-xl text-[13px] text-muted-foreground hover:text-sidebar-foreground font-normal">
          <Terminal className="size-4" />New Terminal
        </Button>
      </div>

      <div className="px-3 pb-3">
        <div className="relative">
          <FolderOpen className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground/30" />
          <Input value={workingDir} onChange={(e) => setWorkingDir(e.target.value)} placeholder="Working directory"
            className="pl-7 h-8 text-[12px] rounded-xl bg-sidebar-accent/30 border-transparent focus:bg-sidebar-accent/50 transition-all duration-200" />
        </div>
      </div>

      <div className="flex-1 min-h-0 flex flex-col px-3 pb-3">
        <div className="flex items-center justify-between mb-2.5 px-1">
          <span className="text-[11px] font-medium text-muted-foreground/40 uppercase tracking-wider">Sessions</span>
          <span className="text-[11px] text-muted-foreground/30 tabular-nums">{sessions.length}</span>
        </div>
        <ScrollArea className="flex-1 -mx-1 px-1">
          {sessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 gap-3 text-muted-foreground/20">
              <Plus className="size-5" />
              <p className="text-[12px]">No sessions yet</p>
            </div>
          ) : (
            <div className="space-y-0.5">
              {sessions.map((s) => {
                const isActive = s.id === activeSessionId;
                const isAgent = s.agentType !== "bash";
                return (
                  <div key={s.id} onClick={() => setActiveSession(s.id)}
                    className={cn(
                      "group flex items-center gap-3 px-3 py-2.5 rounded-xl cursor-pointer transition-all duration-200",
                      isActive
                        ? "bg-sidebar-accent text-sidebar-accent-foreground shadow-sm"
                        : "text-muted-foreground hover:bg-sidebar-accent/40 hover:text-sidebar-foreground"
                    )}>
                    {isAgent ? <Bot className="size-4 shrink-0" /> : <Terminal className="size-4 shrink-0" />}
                    <div className="flex-1 min-w-0">
                      <p className="truncate text-[13px] font-medium capitalize">{s.agentType}</p>
                      <p className="truncate text-[10px] text-muted-foreground/50 mt-0.5">{s.id.slice(0, 8)}</p>
                    </div>
                    <span className={cn("size-1.5 rounded-full shrink-0", s.status === "running" ? "bg-emerald-400" : "bg-muted-foreground/20")} />
                    <span className="opacity-0 group-hover:opacity-100 rounded-lg hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all flex-shrink-0" onClick={(e) => { e.stopPropagation(); kill(s.id); }}>
                      <X className="size-3" />
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>
      </div>

      <div className="px-5 py-2.5 border-t border-sidebar-border/50">
        <p className="text-[10px] text-muted-foreground/30 font-mono">v0.3</p>
      </div>
    </aside>
  );
}
