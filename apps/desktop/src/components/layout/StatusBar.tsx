import { Moon, Sun } from "lucide-react";
import { useStore, useSessionList } from "@/store";

export function StatusBar() {
  const sessions = useSessionList();
  const activeId = useStore((s) => s.activeSessionId);
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const active = sessions.find((s) => s.id === activeId);
  const running = sessions.filter((s) => s.status === "running").length;

  return (
    <footer className="h-8 flex items-center px-4 text-[11px] text-muted-foreground/60 flex-shrink-0 gap-3 select-none border-t border-border/20">
      <span className="flex items-center gap-1.5">
        <span className={`size-1.5 rounded-full ${running > 0 ? "bg-emerald-400/70" : "bg-muted-foreground/20"}`} />
        {running} running
      </span>
      {active && <span className="truncate">· {active.agentType} · {active.model?.replace("claude-","").replace("deepseek-","").replace("-20251001","") || ""}</span>}
      {active && active.costUsd > 0 && <span className="font-mono text-muted-foreground/70">${active.costUsd.toFixed(4)}</span>}
      <div className="flex-1" />
      <button onClick={() => setTheme(theme === "dark" ? "light" : "dark")} className="p-0.5 hover:text-foreground/60 transition-colors">
        {theme === "dark" ? <Sun className="size-3" /> : <Moon className="size-3" />}
      </button>
    </footer>
  );
}
