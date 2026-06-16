import { Activity, Bell, CalendarClock, PanelRightOpen } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { useSchedulerQuery } from "@/hooks/queries/useSchedulerQuery";
import { deriveBackgroundTaskStatus } from "@/lib/backgroundTaskStatus";
import { useStore } from "@/store";

function iconFor(tone: string) {
  if (tone === "scheduler") return CalendarClock;
  if (tone === "alert") return Bell;
  return Activity;
}

export function StatusBar() {
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
      className="forge-background-status-bar"
      data-testid="background-task-status"
      role="status"
      aria-live="polite"
    >
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
  );
}
