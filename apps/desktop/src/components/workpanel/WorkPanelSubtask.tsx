import { useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, ListTree, Send, UserRoundCheck, X } from "lucide-react";
import { AgentA2AFocusedTask } from "@/components/messages/AgentA2ATimeline";
import { reviewAgentA2ATasks, type AgentA2AReviewDecision } from "@/lib/ipc/a2a";
import { runtimeFactsForSubagentTask } from "@/lib/loopRuntime";
import { useStore } from "@/store";
import { runtimeFactSourcesForSubagentTasks } from "@/store/runtime-projections";
import type { WorkPanelTab } from "./workPanelTypes";

type WorkPanelSubtaskTab = Extract<WorkPanelTab, { kind: "subtask" }>;

export function WorkPanelSubtask({ tab }: { tab: WorkPanelSubtaskTab }) {
  const [showInstruction, setShowInstruction] = useState(false);
  const [instruction, setInstruction] = useState("");
  const [reviewBusy, setReviewBusy] = useState<AgentA2AReviewDecision | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const projection = useStore((state) => state.agentA2ABySession.get(tab.taskId) ?? null);
  const runtimeEntries = useStore((state) => state.subagentRuntimeByTask);
  const setPendingInput = useStore((state) => state.setPendingInput);
  const task = projection?.tasks.find((candidate) => candidate.task_id === tab.subtaskId) ?? null;

  if (!task) {
    return (
      <div className="forge-work-panel-placeholder" data-testid="work-panel-content-subtask">
        <ListTree className="size-5" />
        <strong>{tab.label}</strong>
        <span>这个子任务暂时不在当前会话中。</span>
      </div>
    );
  }

  const runtimeSources = runtimeFactSourcesForSubagentTasks({
    entries: runtimeEntries,
    taskIds: new Set([task.task_id]),
    sessionId: tab.taskId,
  });
  const runtimeFacts = runtimeFactsForSubagentTask(runtimeSources, task.task_id);

  const sendInstruction = () => {
    const nextInstruction = instruction.trim();
    if (!nextInstruction) return;
    setPendingInput(`给子任务「${task.title}」补充指令：\n${nextInstruction}`);
    setInstruction("");
    setShowInstruction(false);
  };

  const handleReview = async (decision: AgentA2AReviewDecision) => {
    setReviewBusy(decision);
    setReviewError(null);
    try {
      const next = await reviewAgentA2ATasks({
        sessionId: tab.taskId,
        taskIds: [task.task_id],
        decision,
      });
      useStore.setState((current) => {
        const agentA2ABySession = new Map(current.agentA2ABySession);
        agentA2ABySession.set(next.session_id, next.state);
        return { agentA2ABySession };
      });
    } catch (error) {
      setReviewError(error instanceof Error ? error.message : String(error));
    } finally {
      setReviewBusy(null);
    }
  };

  return (
    <section className="forge-work-panel-subtask" aria-label={`子任务 ${task.title}`}>
      <div className="forge-work-panel-content-toolbar forge-work-panel-subtask-toolbar">
        <div className="forge-work-panel-content-title">
          <ListTree className="size-4" />
          <span>{task.title}</span>
          <small>{task.role}</small>
        </div>
        <div className="forge-work-panel-subtask-actions">
          {task.needs_human_review === true ? (
            <>
              <ButtonPrimitive
                type="button"
                onClick={() => void handleReview("approve")}
                disabled={reviewBusy !== null}
                aria-label={`通过审阅 ${task.title}`}
              >
                <Check className="size-3.5" />
                通过
              </ButtonPrimitive>
              <ButtonPrimitive
                type="button"
                onClick={() => void handleReview("reject")}
                disabled={reviewBusy !== null}
                aria-label={`拒绝审阅 ${task.title}`}
              >
                <X className="size-3.5" />
                拒绝
              </ButtonPrimitive>
            </>
          ) : null}
          <ButtonPrimitive type="button" onClick={() => setShowInstruction((value) => !value)}>
            <Send className="size-3.5" />
            补充指令
          </ButtonPrimitive>
          <ButtonPrimitive
            type="button"
            disabled
            aria-label="接管子任务"
            title="当前运行时未提供子任务接管能力"
          >
            <UserRoundCheck className="size-3.5" />
            接管
          </ButtonPrimitive>
        </div>
      </div>

      {showInstruction ? (
        <div className="forge-work-panel-subtask-instruction">
          <textarea
            autoFocus
            value={instruction}
            onChange={(event) => setInstruction(event.target.value)}
            placeholder="告诉这个子任务接下来要做什么"
          />
          <ButtonPrimitive type="button" disabled={!instruction.trim()} onClick={sendInstruction}>
            发送到对话
          </ButtonPrimitive>
        </div>
      ) : null}
      {reviewError ? <p className="forge-work-panel-inline-error" role="alert">{reviewError}</p> : null}

      <div className="forge-work-panel-subtask-body">
        <AgentA2AFocusedTask task={task} runtimeFacts={runtimeFacts} />
      </div>
    </section>
  );
}
