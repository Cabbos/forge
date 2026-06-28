import { useState, useCallback } from "react";
import { AlertCircle, Check, Loader2, X } from "lucide-react";
import { type MemoryFact } from "@/lib/tauri";
import { formatMutationError } from "./settingsUtils";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function MemoryInlineEditor({
  initial,
  onSave,
  onCancel,
}: {
  initial: MemoryFact;
  onSave: (text: string, tags: string[]) => Promise<void> | void;
  onCancel: () => void;
}) {
  const [text, setText] = useState(initial.text);
  const [tagsStr, setTagsStr] = useState(initial.tags.join(", "));
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleSave = useCallback(async () => {
    const trimmed = text.trim();
    if (!trimmed) return;
    setSaveError(null);
    setSaving(true);
    try {
      const tagList = tagsStr
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
      await onSave(trimmed, tagList);
    } catch (error) {
      setSaveError(formatMutationError(error));
    } finally {
      setSaving(false);
    }
  }, [text, tagsStr, onSave]);

  return (
    <div className="forge-memory-editor">
      <textarea
        className="forge-memory-editor-text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={2}
        placeholder="输入记忆事实…"
        disabled={saving}
      />
      <input
        className="forge-memory-editor-tags"
        value={tagsStr}
        onChange={(e) => setTagsStr(e.target.value)}
        placeholder="标签, 逗号分隔"
        disabled={saving}
      />
      <div className="forge-memory-editor-actions">
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn"
          onClick={handleSave}
          disabled={saving || !text.trim()}
          aria-label="保存"
        >
          {saving ? <Loader2 className="size-3 animate-spin" /> : <Check className="size-3" />}
        </ButtonPrimitive>
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn forge-memory-action-btn--cancel"
          onClick={onCancel}
          disabled={saving}
          aria-label="取消"
        >
          <X className="size-3" />
        </ButtonPrimitive>
      </div>
      {saveError && (
        <div className="forge-memory-editor-error" role="alert">
          <AlertCircle className="size-3" />
          <span>{saveError}</span>
        </div>
      )}
    </div>
  );
}
