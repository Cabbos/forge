import { CheckCircle2, CircleDashed, XCircle, PauseCircle } from "lucide-react";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "@/lib/protocol";

function iconFor(status: string) {
  if (status === "completed") return CheckCircle2;
  if (status === "failed") return XCircle;
  if (status === "interrupted") return PauseCircle;
  return CircleDashed;
}

function TaskRow({ task }: { task: AgentA2ATaskProjection }) {
  const Icon = iconFor(task.status);
  return (
    <div className="forge-a2a-task-row" data-status={task.status}>
      <Icon className="size-3" />
      <span className="forge-a2a-task-title">{task.title}</span>
      <span className="forge-a2a-task-role">{task.role}</span>
      {task.latest_message && (
        <span className="forge-a2a-task-message">{task.latest_message}</span>
      )}
      {task.failure_message && (
        <span className="forge-a2a-task-failure">{task.failure_message}</span>
      )}
    </div>
  );
}

export function AgentA2ATimeline({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  return (
    <div className="forge-a2a-timeline" data-testid="agent-a2a-timeline">
      <div className="forge-a2a-summary">
        <span>子任务</span>
        <span>{state.running_count} 运行中</span>
        <span>{state.completed_count} 完成</span>
        {state.failed_count > 0 && <span>{state.failed_count} 失败</span>}
      </div>
      <div className="forge-a2a-task-list">
        {state.tasks.map((task) => (
          <TaskRow key={task.task_id} task={task} />
        ))}
      </div>
    </div>
  );
}
