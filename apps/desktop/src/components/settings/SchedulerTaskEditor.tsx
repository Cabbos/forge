import { useState, useCallback } from "react";
import { AlertCircle, Check, Loader2, X } from "lucide-react";
import { type ScheduledTask } from "@/lib/tauri";
import { formatMutationError } from "./settingsUtils";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function SchedulerTaskEditor({
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
        <ButtonPrimitive
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
        </ButtonPrimitive>
        <ButtonPrimitive
          type="button"
          className="forge-scheduler-action-btn forge-scheduler-action-btn--cancel"
          onClick={onCancel}
          disabled={saving}
          aria-label="取消"
        >
          <X className="size-3" />
        </ButtonPrimitive>
      </div>
    </div>
  );
}
