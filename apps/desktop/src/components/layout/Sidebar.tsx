import { useState, useEffect } from "react";
import { X, Plus, FolderOpen } from "lucide-react";
import { useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
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

  const newSession = async () => {
    try { await create(workingDir); }
    catch (e) { alert("Failed to create session:\n" + String(e)); }
  };

  return (
    <aside className="h-full flex flex-col select-none" style={{ background: "#090909", borderRight: "1px solid #161616" }}>
      {/* Brand */}
      <div className="flex items-center justify-between px-4 py-5">
        <div className="flex items-center gap-2.5">
          {/* DeepSeek whale icon */}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
            <path d="M12 3C8 3 4 7 3 11C2 13 2 16 4 17.5C5 18.5 7 18 8 17C9 16 10 14.5 12 14.5C14 14.5 15 16 16 17C17 18 19 18.5 20 17.5C22 16 22 13 21 11C20 7 16 3 12 3Z"
              fill="#4B9CD3" opacity="0.9" />
            <path d="M5 13C4 14 4.5 15 6 15.5" stroke="#4B9CD3" strokeWidth="1.5" strokeLinecap="round" opacity="0.5"/>
            <circle cx="9" cy="8" r="1.2" fill="#fff" opacity="0.7"/>
          </svg>
          <h1 className="text-sm font-semibold tracking-tight" style={{ color: "#E4E4E4" }}>Deep Agent</h1>
        </div>
        <SettingsDialog />
      </div>

      {/* New Session */}
      <div className="px-3 pb-4">
        <button onClick={newSession}
          className="w-full flex items-center gap-2.5 pl-3 pr-4 py-2.5 rounded-xl text-sm font-medium transition-all"
          style={{ background: "#141414", border: "1px solid #1c1c1c", color: "#b0b0b0" }}>
          <span className="text-base" style={{ color: "#D4A853" }}>+</span>
          New Session
        </button>
      </div>

      {/* Working dir */}
      <div className="px-3 pb-4">
        <div className="relative">
          <FolderOpen className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5" style={{ color: "#444" }} />
          <Input value={workingDir} onChange={(e) => setWorkingDir(e.target.value)}
            className="pl-7 h-8 text-[11px] rounded-lg border-0"
            style={{ background: "#0f0f0f", color: "#b0b0b0" }} />
        </div>
      </div>

      {/* Sessions */}
      <div className="flex-1 min-h-0 flex flex-col px-3 pb-3">
        <div className="flex items-center justify-between mb-2.5 px-1">
          <span className="text-[10px] font-medium uppercase tracking-widest" style={{ color: "#444" }}>Sessions</span>
          <span className="text-[10px] tabular-nums" style={{ color: "#555" }}>{sessions.length}</span>
        </div>
        <ScrollArea className="flex-1 -mx-1 px-1">
          {sessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-20 gap-3" style={{ color: "#3a3a3a" }}>
              <Plus className="size-5" />
              <p className="text-[11px]">No sessions yet</p>
            </div>
          ) : (
            <div className="space-y-0.5">
              {sessions.map((s) => {
                const isActive = s.id === activeSessionId;
                return (
                  <div key={s.id} onClick={() => setActiveSession(s.id)}
                    className={cn(
                      "group flex items-center gap-2.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all",
                      isActive ? "text-[#E4E4E4]" : "text-[#777] hover:text-[#aaa]"
                    )}
                    style={{ background: isActive ? "#121212" : "transparent" }}>
                    <span className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                      style={{ background: s.status === "running" ? "#D4A853" : "#333" }} />
                    <div className="flex-1 min-w-0 flex items-center gap-1.5">
                      <span className="text-[11px] truncate">{s.agentType || "deepseek"}</span>
                      <span className="text-[10px] font-mono" style={{ color: "#555" }}>{s.id.slice(0, 8)}</span>
                    </div>
                    <span onClick={(e) => { e.stopPropagation(); kill(s.id); }}
                      className="opacity-0 group-hover:opacity-60 hover:opacity-100 rounded p-0.5 transition-all flex-shrink-0"
                      style={{ color: "#777" }}>
                      <X className="size-3" />
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>
      </div>

      {/* Version */}
      <div className="px-5 py-2.5" style={{ borderTop: "1px solid #151515" }}>
        <p className="text-[9px] font-mono" style={{ color: "#333" }}>v0.4 · DeepSeek</p>
      </div>
    </aside>
  );
}
