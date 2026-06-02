import { useRef } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Trash2 } from "lucide-react";
import type { SessionState } from "@/lib/protocol";
import { getSessionTitle } from "@/lib/session-display";
import { cn } from "@/lib/utils";

interface SidebarSessionHistoryProps {
  activeSessionId: string | null;
  onDeleteSession: (sessionId: string) => void;
  onSelectSession: (sessionId: string) => void;
  sessions: SessionState[];
}

export function SidebarSessionHistory({
  activeSessionId,
  onDeleteSession,
  onSelectSession,
  sessions,
}: SidebarSessionHistoryProps) {
  const sessionRowRefs = useRef(new Map<string, HTMLDivElement>());
  const groupedSessions = groupSessionsByRecency(sessions);

  const focusSessionAt = (index: number) => {
    const nextSession = sessions[index];
    if (!nextSession) return;
    sessionRowRefs.current.get(nextSession.id)?.focus();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div data-forge-motion="sidebar-entry" className="mb-1.5 flex items-center justify-between px-1">
        <span className="forge-sidebar-section-title">对话</span>
      </div>
      <div className="forge-sidebar-history-list flex-1 overflow-y-auto">
        {groupedSessions.map((group) => (
          <div key={group.label} data-forge-motion="sidebar-entry" className="forge-sidebar-history-group">
            <div className="forge-sidebar-history-group-label">
              {group.label}
            </div>
            {group.sessions.map((session) => {
              const index = sessions.findIndex((item) => item.id === session.id);
              const isActive = session.id === activeSessionId;
              const title = getSessionTitle(session);
              return (
                <div
                  key={session.id}
                  ref={(node) => {
                    if (node) sessionRowRefs.current.set(session.id, node);
                    else sessionRowRefs.current.delete(session.id);
                  }}
                  role="button"
                  aria-label={title}
                  data-active={isActive ? "true" : "false"}
                  tabIndex={0}
                  onClick={() => onSelectSession(session.id)}
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
                      onSelectSession(session.id);
                    }
                  }}
                  className={cn("forge-sidebar-history-row group", isActive ? "text-sidebar-accent-foreground" : "text-muted-foreground")}
                >
                  <span className={cn("min-w-0 flex-1 truncate", isActive && "font-medium")}>{title}</span>
                  <ButtonPrimitive
                    type="button"
                    aria-label={`删除对话 ${title}`}
                    className="forge-sidebar-history-delete"
                    onClick={(event) => {
                      event.stopPropagation();
                      onDeleteSession(session.id);
                    }}
                  >
                    <Trash2 className="size-3" />
                  </ButtonPrimitive>
                </div>
              );
            })}
          </div>
        ))}
        {sessions.length === 0 && (
          <p data-forge-motion="sidebar-entry" className="forge-sidebar-empty-state">
            还没有对话
          </p>
        )}
      </div>
    </div>
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
