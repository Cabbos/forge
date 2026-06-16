import { useState, useCallback } from "react";
import { AlertCircle, Circle, CircleCheck, Loader2, Pencil, Trash2 } from "lucide-react";
import {
  type ForgeProfile,
  type UpsertProfileInput,
} from "@/lib/tauri";
import { formatMutationError, formatTimestamp } from "./settingsUtils";
import { ProfileForm } from "./ProfileForm";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function ProfileRow({
  profile,
  isActive,
  onSetActive,
  onDelete,
  onUpdate,
}: {
  profile: ForgeProfile;
  isActive: boolean;
  onSetActive: (id: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onUpdate: (input: UpsertProfileInput) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const isDefault = profile.id === "default";

  const handleDelete = useCallback(async () => {
    setDeleteError(null);
    setDeleting(true);
    try {
      await onDelete(profile.id);
    } catch (error) {
      setDeleteError(formatMutationError(error));
    } finally {
      setDeleting(false);
    }
  }, [profile.id, onDelete]);

  const meta: string[] = [];
  if (profile.default_provider) meta.push(profile.default_provider);
  if (profile.default_model) meta.push(profile.default_model);
  if (profile.default_workspace) meta.push(profile.default_workspace);

  if (editing) {
    return (
      <div className="forge-memory-fact" data-editing="true">
        <ProfileForm
          initial={profile}
          submitLabel="更新"
          onSave={async (input) => {
            await onUpdate({ ...input, id: profile.id });
            setEditing(false);
          }}
          onCancel={() => setEditing(false)}
        />
      </div>
    );
  }

  return (
    <div className="forge-memory-fact" data-active={isActive ? "true" : "false"}>
      <div className="forge-memory-fact-body">
        <div className="forge-memory-fact-meta" style={{ gap: 4 }}>
          <ButtonPrimitive
            type="button"
            className="forge-memory-action-btn"
            onClick={() => onSetActive(profile.id)}
            aria-label={isActive ? "当前活跃" : "设为活跃"}
            title={isActive ? "当前活跃" : "设为活跃"}
          >
            {isActive ? (
              <CircleCheck className="size-3.5" style={{ color: "var(--forge-active-text, #8B6914)" }} />
            ) : (
              <Circle className="size-3.5" />
            )}
          </ButtonPrimitive>
          <p className="forge-memory-fact-text" style={{ margin: 0, fontWeight: 600 }}>
            {profile.name}
          </p>
          {isDefault && (
            <span className="forge-memory-tag" style={{ fontSize: 10 }}>默认</span>
          )}
        </div>
        {meta.length > 0 && (
          <div className="forge-memory-fact-meta" style={{ paddingLeft: 24 }}>
            <span className="forge-memory-fact-source">{meta.join(" · ")}</span>
          </div>
        )}
        <div className="forge-memory-fact-meta" style={{ paddingLeft: 24 }}>
          <span className="forge-memory-fact-time">{formatTimestamp(profile.updated_at_ms)}</span>
        </div>
      </div>
      <div className="forge-memory-fact-actions">
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn"
          onClick={() => setEditing(true)}
          aria-label="编辑"
        >
          <Pencil className="size-3" />
        </ButtonPrimitive>
        {!isDefault && !isActive && (
          <ButtonPrimitive
            type="button"
            className="forge-memory-action-btn forge-memory-action-btn--danger"
            onClick={handleDelete}
            disabled={deleting}
            aria-label="删除"
          >
            {deleting ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Trash2 className="size-3" />
            )}
          </ButtonPrimitive>
        )}
      </div>
      {deleteError && (
        <div className="forge-memory-row-error" role="alert">
          <AlertCircle className="size-3" />
          <span>{deleteError}</span>
        </div>
      )}
    </div>
  );
}
