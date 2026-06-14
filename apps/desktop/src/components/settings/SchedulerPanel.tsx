import { useState, useCallback } from "react";
import {
  Clock,
  Plus,
  Trash2,
  Play,
  Pause,
  RefreshCw,
  Pencil,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useSchedulerQuery } from "@/hooks/queries/useSchedulerQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import {
  upsertScheduledTask,
  deleteScheduledTask,
  setScheduledTaskEnabled,
  runScheduledTaskNow,
} from "@/lib/tauri";
import { formatMutationError, formatInterval, formatTimestamp } from "./settingsUtils";
import { SchedulerTaskEditor } from "./SchedulerTaskEditor";
import { SchedulerHistoryRow } from "./SchedulerHistoryRow";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function SchedulerPanel() {
  const queryClient = useQueryClient();
  const { data, isLoading, isError, error, refetch } = useSchedulerQuery();
  const [editing, setEditing] = useState<string | null>(null); // task id being edited, or "new"
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [runningIds, setRunningIds] = useState<Set<string>>(new Set());

  const invalidate = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: queryKeys.schedulerAll });
  }, [queryClient]);

  const handleUpsert = useCallback(
    async (input: {
      id?: string;
      title: string;
      text: string;
      tags: string[];
      interval_seconds: number;
      profile_id?: string;
    }) => {
      setMutationError(null);
      await upsertScheduledTask({
        id: input.id ?? null,
        title: input.title,
        text: input.text,
        tags: input.tags,
        interval_seconds: input.interval_seconds,
        profile_id: input.profile_id ?? null,
      });
      setEditing(null);
      invalidate();
    },
    [invalidate],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      setMutationError(null);
      try {
        await deleteScheduledTask(id);
        invalidate();
      } catch (err) {
        setMutationError(formatMutationError(err));
      }
    },
    [invalidate],
  );

  const handleToggle = useCallback(
    async (id: string, enabled: boolean) => {
      setMutationError(null);
      try {
        await setScheduledTaskEnabled(id, enabled);
        invalidate();
      } catch (err) {
        setMutationError(formatMutationError(err));
      }
    },
    [invalidate],
  );

  const handleRunNow = useCallback(
    async (id: string) => {
      setMutationError(null);
      setRunningIds((prev) => new Set(prev).add(id));
      try {
        await runScheduledTaskNow(id);
        invalidate();
      } finally {
        setRunningIds((prev) => {
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
      }
    },
    [invalidate],
  );

  // ── Render ──────────────────────────────────────────────────────────────

  if (isLoading) {
    return (
      <div className="forge-scheduler-panel">
        <div className="forge-scheduler-loading" role="status">
          <Loader2 className="size-4 animate-spin" />
          <span>加载任务列表…</span>
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="forge-scheduler-panel">
        <div className="forge-scheduler-error" role="alert">
          <AlertCircle className="size-4" />
          <span>{getQueryErrorMessage(error)}</span>
          <ButtonPrimitive
            type="button"
            className="forge-scheduler-retry-btn"
            onClick={() => refetch()}
          >
            重试
          </ButtonPrimitive>
        </div>
      </div>
    );
  }

  const tasks = data?.tasks ?? [];
  const history = data?.recent_history ?? [];
  const loadError = data?.load_error;

  return (
    <div className="forge-scheduler-panel">
      {/* Load error banner */}
      {loadError && (
        <div className="forge-scheduler-banner" role="alert">
          <AlertCircle className="size-3.5" />
          <span>调度器数据文件已损坏: {loadError}。编辑后会自动覆盖。</span>
        </div>
      )}

      {/* Toolbar */}
      <div className="forge-scheduler-toolbar">
        <ButtonPrimitive
          type="button"
          className="forge-scheduler-create-btn"
          onClick={() => setEditing("new")}
        >
          <Plus className="size-3.5" />
          <span>新建任务</span>
        </ButtonPrimitive>
        <ButtonPrimitive
          type="button"
          className="forge-scheduler-refresh-btn"
          onClick={() => refetch()}
          aria-label="刷新"
        >
          <RefreshCw className="size-3.5" />
        </ButtonPrimitive>
      </div>

      {/* New task editor */}
      {editing === "new" && (
        <SchedulerTaskEditor
          onSave={handleUpsert}
          onCancel={() => setEditing(null)}
        />
      )}

      {/* Mutation error */}
      {mutationError && (
        <div className="forge-scheduler-mutation-error" role="alert">
          <AlertCircle className="size-3" />
          <span>{mutationError}</span>
        </div>
      )}

      {/* Task list */}
      {tasks.length === 0 && !editing ? (
        <div className="forge-scheduler-empty">
          <Clock className="size-5" />
          <p>尚无定时任务。点击「新建任务」创建。</p>
        </div>
      ) : (
        <ul className="forge-scheduler-task-list">
          {tasks.map((task) => {
            const isEditing = editing === task.id;
            const isRunning = runningIds.has(task.id);

            return (
              <li key={task.id} className="forge-scheduler-task-card">
                {isEditing ? (
                  <SchedulerTaskEditor
                    initial={task}
                    onSave={handleUpsert}
                    onCancel={() => setEditing(null)}
                  />
                ) : (
                  <>
                    <div className="forge-scheduler-task-header">
                      <div className="forge-scheduler-task-title-row">
                        <h4 className="forge-scheduler-task-title">
                          {task.title}
                        </h4>
                        {!task.enabled && (
                          <span className="forge-scheduler-disabled-pill">
                            已禁用
                          </span>
                        )}
                      </div>
                      <div className="forge-scheduler-task-actions">
                        <ButtonPrimitive
                          type="button"
                          className="forge-scheduler-icon-btn"
                          onClick={() => handleToggle(task.id, !task.enabled)}
                          aria-label={task.enabled ? "禁用" : "启用"}
                          title={task.enabled ? "禁用" : "启用"}
                        >
                          {task.enabled ? (
                            <Pause className="size-3.5" />
                          ) : (
                            <Play className="size-3.5" />
                          )}
                        </ButtonPrimitive>
                        <ButtonPrimitive
                          type="button"
                          className="forge-scheduler-icon-btn"
                          onClick={() => handleRunNow(task.id)}
                          disabled={isRunning}
                          aria-label="立即运行"
                          title="立即运行"
                        >
                          {isRunning ? (
                            <Loader2 className="size-3.5 animate-spin" />
                          ) : (
                            <Play className="size-3.5" />
                          )}
                        </ButtonPrimitive>
                        <ButtonPrimitive
                          type="button"
                          className="forge-scheduler-icon-btn"
                          onClick={() => setEditing(task.id)}
                          aria-label="编辑"
                          title="编辑"
                        >
                          <Pencil className="size-3.5" />
                        </ButtonPrimitive>
                        <ButtonPrimitive
                          type="button"
                          className="forge-scheduler-icon-btn forge-scheduler-icon-btn--danger"
                          onClick={() => handleDelete(task.id)}
                          aria-label="删除"
                          title="删除"
                        >
                          <Trash2 className="size-3.5" />
                        </ButtonPrimitive>
                      </div>
                    </div>

                    <div className="forge-scheduler-task-body">
                      <p className="forge-scheduler-task-text">{task.text}</p>

                      <div className="forge-scheduler-task-meta">
                        <span className="forge-scheduler-meta-item">
                          间隔: {formatInterval(task.interval_seconds)}
                        </span>
                        <span className="forge-scheduler-meta-item">
                          下次运行:{" "}
                          {task.interval_seconds === 0
                            ? "手动触发"
                            : formatTimestamp(task.next_run_at_ms)}
                        </span>
                        {task.last_run_at_ms != null && (
                          <span className="forge-scheduler-meta-item">
                            上次运行: {formatTimestamp(task.last_run_at_ms)}
                          </span>
                        )}
                        {task.last_error && (
                          <span className="forge-scheduler-meta-item forge-scheduler-meta-item--error">
                            错误: {task.last_error}
                          </span>
                        )}
                      </div>

                      {task.tags.length > 0 && (
                        <div className="forge-scheduler-task-tags">
                          {task.tags.map((tag) => (
                            <span
                              key={tag}
                              className="forge-scheduler-tag"
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>

                    {/* Per-task history */}
                    {(() => {
                      const taskHistory = history.filter(
                        (h) => h.task_id === task.id,
                      );
                      if (taskHistory.length === 0) return null;
                      return (
                        <details className="forge-scheduler-task-history">
                          <summary className="forge-scheduler-history-summary">
                            最近运行记录 ({taskHistory.length})
                          </summary>
                          <ul className="forge-scheduler-history-list">
                            {taskHistory.slice(0, 5).map((entry) => (
                              <SchedulerHistoryRow key={entry.id} entry={entry} />
                            ))}
                          </ul>
                        </details>
                      );
                    })()}
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {/* Global history footer */}
      {history.length > 0 && (
        <details className="forge-scheduler-global-history">
          <summary className="forge-scheduler-history-summary">
            全部运行记录 ({history.length})
          </summary>
          <ul className="forge-scheduler-history-list">
            {history.map((entry) => (
              <SchedulerHistoryRow key={entry.id} entry={entry} />
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
