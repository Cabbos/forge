import { useState, useCallback } from "react";
import { AlertCircle, Check, Loader2, X } from "lucide-react";
import {
  type ForgeProfile,
  type UpsertProfileInput,
} from "@/lib/tauri";
import { formatMutationError } from "./settingsUtils";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function ProfileForm({
  initial,
  onSave,
  onCancel,
  submitLabel,
}: {
  initial: ForgeProfile;
  onSave: (input: UpsertProfileInput) => Promise<void>;
  onCancel: () => void;
  submitLabel: string;
}) {
  const [name, setName] = useState(initial.name);
  const [provider, setProvider] = useState(initial.default_provider ?? "");
  const [model, setModel] = useState(initial.default_model ?? "");
  const [workspace, setWorkspace] = useState(initial.default_workspace ?? "");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleSave = useCallback(async () => {
    const trimmedName = name.trim();
    if (!trimmedName) return;
    setSaveError(null);
    setSaving(true);
    try {
      await onSave({
        id: initial.id || null,
        name: trimmedName,
        default_provider: provider.trim() || null,
        default_model: model.trim() || null,
        default_workspace: workspace.trim() || null,
      });
    } catch (error) {
      setSaveError(formatMutationError(error));
    } finally {
      setSaving(false);
    }
  }, [name, provider, model, workspace, initial.id, onSave]);

  return (
    <div className="forge-memory-editor">
      <input
        className="forge-memory-editor-text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="资料名称"
        disabled={saving}
      />
      <input
        className="forge-memory-editor-text"
        value={provider}
        onChange={(e) => setProvider(e.target.value)}
        placeholder="默认服务 (可选)"
        disabled={saving}
      />
      <input
        className="forge-memory-editor-text"
        value={model}
        onChange={(e) => setModel(e.target.value)}
        placeholder="默认模型 (可选)"
        disabled={saving}
      />
      <input
        className="forge-memory-editor-text"
        value={workspace}
        onChange={(e) => setWorkspace(e.target.value)}
        placeholder="默认工作区 (可选)"
        disabled={saving}
      />
      <div className="forge-memory-editor-actions">
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn"
          onClick={handleSave}
          disabled={saving || !name.trim()}
          aria-label={submitLabel}
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
        <span className="forge-memory-fact-time">{submitLabel}</span>
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
