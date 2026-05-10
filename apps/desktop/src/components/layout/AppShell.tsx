import { useStore } from "@/store";
import { Sidebar } from "./Sidebar";
import { SessionView } from "@/components/session/SessionView";
import { HubPanel } from "./HubPanel";
import { useOutputStream } from "@/hooks/useOutputStream";

export function AppShell() {
  const activeSessionId = useStore((s) => s.activeSessionId);
  const sessions = useStore((s) => s.sessions);
  useOutputStream(activeSessionId);

  return (
    <div className="h-screen grid bg-background" style={{ gridTemplateColumns: "240px 1fr 320px" }}>
      <Sidebar />
      <main className="flex flex-col h-full min-w-0 overflow-hidden border-r border-border">
        {activeSessionId && sessions.has(activeSessionId) ? (
          <SessionView sessionId={activeSessionId} />
        ) : (
          <div className="flex flex-col items-center justify-center h-full gap-4" style={{ color: "#555" }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" opacity="0.3">
              <path d="M12 3C8 3 4 7 3 11C2 13 2 16 4 17.5C5 18.5 7 18 8 17C9 16 10 14.5 12 14.5C14 14.5 15 16 16 17C17 18 19 18.5 20 17.5C22 16 22 13 21 11C20 7 16 3 12 3Z"
                fill="#4B9CD3" />
              <circle cx="9" cy="8" r="1.2" fill="#0D0D0D"/>
            </svg>
            <p className="text-sm">Create a session to begin</p>
          </div>
        )}
      </main>
      <HubPanel />
    </div>
  );
}
