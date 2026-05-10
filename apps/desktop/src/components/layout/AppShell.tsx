import { useEffect } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { useStore } from "@/store";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { SessionView } from "@/components/session/SessionView";
import { useOutputStream } from "@/hooks/useOutputStream";

export function AppShell() {
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  const theme = useStore((s) => s.theme);

  useOutputStream(activeSessionId);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  return (
    <div className="flex flex-col h-screen bg-background text-foreground">
      <Group orientation="horizontal" className="flex-1">
        <Panel defaultSize="16" minSize="14" maxSize="28">
          <Sidebar />
        </Panel>
        <Separator className="w-[2px] bg-transparent hover:bg-primary/10 transition-colors duration-300 cursor-col-resize" />
        <Panel defaultSize="84" minSize="35">
          <main className="flex flex-col h-full min-w-0">
            {activeSessionId && sessions.has(activeSessionId) ? (
              <SessionView sessionId={activeSessionId} />
            ) : (
              <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground/30">
                <div className="size-12 rounded-2xl bg-muted/30 flex items-center justify-center">
                  <div className="size-5 rounded-lg border-2 border-muted-foreground/20" />
                </div>
                <p className="text-sm">Create a new session to begin</p>
              </div>
            )}
          </main>
        </Panel>
      </Group>
      <StatusBar />
    </div>
  );
}
