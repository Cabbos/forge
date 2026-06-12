import { useState, useCallback } from "react";
import {
  Clock,
  Plus,
  Trash2,
  Play,
  Pause,
  RefreshCw,
  Pencil,
  Check,
  X,
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
  type ScheduledTask,
  type RunHistoryEntry,
} from "@/lib/tauri";

// ── Formatters ────────────────────────────────────────────────────────────────

function formatTimestamp(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function formatInterval(seconds: number): string {
  if (seconds === 0) return "手动";
  if (seconds < 60) return `${seconds} 秒`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 时`;
  return `${Math.floor(seconds / 86400)} 天`;
}

function formatMutationError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "操作失败，请重试。";
}

// ── Status badge ─────────────────────────────────────────────────────────────

function HistoryStatusBadge({ status }: { status: string }) {
  const style =
    status === "completed"
      ? { bg: "var(--forge-active)", fg: "var(--forge-text-primary)" }
      : status === "skipped"
        ? { bg: "rgba(184, 138, 86, 0.15)", fg: "var(--forge-text-muted)" }
        : { bg: "rgba(220, 80, 60, 0.12)", fg: "#b33a2e" };

  const label =
    status === "completed" ? "完成" : status === "skipped" ? "跳过" : "错误";

  return (
    <span
      className="forge-scheduler-status-badge"
      style={{ background: style.bg, color: style.fg }}
    >
      {label}
    </span>
  );
}

// ── Task editor sub-component ─────────────────────────────────────────────────

function TaskEditor({
  initial,
  onSave,
  onCancel,
}: {
  initial?: ScheduledTask;
  onSave: (data: {
    id?: string;
    title: string;
    text: string;
    tags: string[];
    interval_seconds: number;
    profile_id?: string;
  }) => Promise<void>;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [text, setText] = useState(initial?.text ?? "");
  const [tagsStr, setTagsStr] = useState(initial?.tags.join(", ") ?? "");
  const [intervalSecs, setIntervalSecs] = useState(
    String(initial?.interval_seconds ?? 3600),
  );
  const [profileId, setProfileId] = useState(initial?.profile_id ?? "");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleSave = useCallback(async () => {
    const trimmedTitle = title.trim();
    if (!trimmedTitle) return;
    setSaveError(null);
    setSaving(true);
    try {
      const parsedInterval = Math.max(0, parseInt(intervalSecs, 10) || 0);
      const tagList = tagsStr
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
      await onSave({
        id: initial?.id,
        title: trimmedTitle,
        text: text.trim(),
        tags: tagList,
        interval_seconds: parsedInterval,
        profile_id: profileId.trim() || undefined,
      });
    } catch (error) {
      setSaveError(formatMutationError(error));
    } finally {
      setSaving(false);
    }
  }, [title, text, tagsStr, intervalSecs, profileId, initial, onSave]);

  return (
    <div className="forge-scheduler-editor">
      <input
        className="forge-scheduler-editor-title"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="任务名称"
        disabled={saving}
      />
      <textarea
        className="forge-scheduler-editor-text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={3}
        placeholder="提示词 / 命令文本"
        disabled={saving}
      />
      <div className="forge-scheduler-editor-row">
        <input
          className="forge-scheduler-editor-interval"
          value={intervalSecs}
          onChange={(e) => setIntervalSecs(e.target.value)}
          placeholder="间隔（秒），0 为手动"
          disabled={saving}
          type="number"
          min="0"
        />
        <input
          className="forge-scheduler-editor-profile"
          value={profileId}
          onChange={(e) => setProfileId(e.target.value)}
          placeholder="关联资料 ID（可选）"
          disabled={saving}
        />
      </div>
      <input
        className="forge-scheduler-editor-tags"
        value={tagsStr}
        onChange={(e) => setTagsStr(e.target.value)}
        placeholder="标签, 逗号分隔"
        disabled={saving}
      />
      {saveError && (
        <div className="forge-scheduler-editor-error" role="alert">
          <AlertCircle className="size-3" />
          <span>{saveError}</span>
        </div>
      )}
      <div className="forge-scheduler-editor-actions">
        <button
          type="button"
          className="forge-scheduler-action-btn"
          onClick={handleSave}
          disabled={saving || !title.trim()}
          aria-label="保存"
        >
          {saving ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Check className="size-3" />
          )}
        </button>
        <button
          type="button"
          className="forge-scheduler-action-btn forge-scheduler-action-btn--cancel"
          onClick={onCancel}
          disabled={saving}
          aria-label="取消"
        >
          <X className="size-3" />
        </button>
      </div>
    </div>
  );
}

// ── History row ───────────────────────────────────────────────────────────────

function HistoryRow({ entry }: { entry: RunHistoryEntry }) {
  return (
    <li className="forge-scheduler-history-item">
      <HistoryStatusBadge status={entry.status} />
      <span className="forge-scheduler-history-time">
        {formatTimestamp(entry.started_at_ms)}
      </span>
      <span className="forge-scheduler-history-msg">{entry.message}</span>
    </li>
  );
}

// ── Panel ────────────────────────────────────────────────────────────────────

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
          <button
            type="button"
            className="forge-scheduler-retry-btn"
            onClick={() => refetch()}
          >
            重试
          </button>
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
        <button
          type="button"
          className="forge-scheduler-create-btn"
          onClick={() => setEditing("new")}
        >
          <Plus className="size-3.5" />
          <span>新建任务</span>
        </button>
        <button
          type="button"
          className="forge-scheduler-refresh-btn"
          onClick={() => refetch()}
          aria-label="刷新"
        >
          <RefreshCw className="size-3.5" />
        </button>
      </div>

      {/* New task editor */}
      {editing === "new" && (
        <TaskEditor
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
                  <TaskEditor
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
                        <button
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
                        </button>
                        <button
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
                        </button>
                        <button
                          type="button"
                          className="forge-scheduler-icon-btn"
                          onClick={() => setEditing(task.id)}
                          aria-label="编辑"
                          title="编辑"
                        >
                          <Pencil className="size-3.5" />
                        </button>
                        <button
                          type="button"
                          className="forge-scheduler-icon-btn forge-scheduler-icon-btn--danger"
                          onClick={() => handleDelete(task.id)}
                          aria-label="删除"
                          title="删除"
                        >
                          <Trash2 className="size-3.5" />
                        </button>
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
                              <HistoryRow key={entry.id} entry={entry} />
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
              <HistoryRow key={entry.id} entry={entry} />
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
