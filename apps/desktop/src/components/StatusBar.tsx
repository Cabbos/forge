import { useState } from "react";
import { Activity, Bell, CalendarClock, ChevronDown, ChevronUp, PanelRightOpen } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { useSchedulerQuery } from "@/hooks/queries/useSchedulerQuery";
import { deriveBackgroundTaskStatus } from "@/lib/backgroundTaskStatus";
import { useStore } from "@/store";
import { TaskManager } from "@/components/tasks/TaskManager";

function iconFor(tone: string) {
  if (tone === "scheduler") return CalendarClock;
  if (tone === "alert") return Bell;
  return Activity;
}

export function StatusBar() {
  const [expanded, setExpanded] = useState(false);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const agentA2A = useStore((s) =>
    activeSessionId ? s.agentA2ABySession.get(activeSessionId) ?? null : null,
  );
  const healthAlerts = useStore((s) => s.healthAlerts);
  const { data: scheduler } = useSchedulerQuery();
  const view = deriveBackgroundTaskStatus({
    agentA2A,
    scheduler,
    healthAlerts,
  });

  if (!view.visible) return null;

  const openAgentWorkbench = () => {
    window.dispatchEvent(new CustomEvent("open-hub", { detail: { section: "agents" } }));
  };

  return (
    <div
      className="forge-background-status"
      data-testid="background-task-status"
    >
      {expanded && <TaskManager tasks={view.tasks} notifications={view.notifications} />}
      <div className="forge-background-status-bar" role="status" aria-live="polite">
        <span className="forge-background-status-title">
          <Activity className="size-3.5" />
          后台
        </span>
        <div className="forge-background-status-items">
          {view.items.map((item) => {
            const Icon = iconFor(item.tone);
            return (
              <span
                key={item.key}
                className="forge-background-status-item"
                data-tone={item.tone}
              >
                <Icon className="size-3" />
                {item.label}
              </span>
            );
          })}
        </div>
        <ButtonPrimitive
          type="button"
          className="forge-background-status-action"
          aria-label={expanded ? "收起后台任务列表" : "展开后台任务列表"}
          aria-expanded={expanded}
          title={expanded ? "收起后台任务列表" : "展开后台任务列表"}
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded ? <ChevronDown className="size-3.5" /> : <ChevronUp className="size-3.5" />}
        </ButtonPrimitive>
        {view.hasAgentWork && (
          <ButtonPrimitive
            type="button"
            className="forge-background-status-action"
            aria-label="打开后台任务面板"
            title="打开后台任务面板"
            onClick={openAgentWorkbench}
          >
            <PanelRightOpen className="size-3.5" />
          </ButtonPrimitive>
        )}
      </div>
    </div>
  );
}
