import { useState, useCallback } from "react";
import {
  UserRound,
  Plus,
  Trash2,
  RefreshCw,
  Pencil,
  Check,
  X,
  AlertCircle,
  Loader2,
  Circle,
  CircleCheck,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useProfilesQuery } from "@/hooks/queries/useProfilesQuery";
import { queryKeys } from "@/hooks/queries/queryKeys";
import {
  upsertProfile,
  deleteProfile,
  setActiveProfile,
  type ForgeProfile,
  type UpsertProfileInput,
} from "@/lib/tauri";

// ── Helpers ────────────────────────────────────────────────────────────────────

function formatTs(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

// ── Create / Edit form ─────────────────────────────────────────────────────────

function ProfileForm({
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
        <button
          type="button"
          className="forge-memory-action-btn"
          onClick={handleSave}
          disabled={saving || !name.trim()}
          aria-label={submitLabel}
        >
          {saving ? <Loader2 className="size-3 animate-spin" /> : <Check className="size-3" />}
        </button>
        <button
          type="button"
          className="forge-memory-action-btn forge-memory-action-btn--cancel"
          onClick={onCancel}
          disabled={saving}
          aria-label="取消"
        >
          <X className="size-3" />
        </button>
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

// ── Profile row ────────────────────────────────────────────────────────────────

function ProfileRow({
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
          <button
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
          </button>
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
          <span className="forge-memory-fact-time">{formatTs(profile.updated_at_ms)}</span>
        </div>
      </div>
      <div className="forge-memory-fact-actions">
        <button
          type="button"
          className="forge-memory-action-btn"
          onClick={() => setEditing(true)}
          aria-label="编辑"
        >
          <Pencil className="size-3" />
        </button>
        {!isDefault && !isActive && (
          <button
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
          </button>
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

// ── Panel ──────────────────────────────────────────────────────────────────────

export function ProfilesPanel() {
  const [creating, setCreating] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const {
    data: payload,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useProfilesQuery();

  const invalidate = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.profilesAll });
  }, [queryClient]);

  const profiles = payload?.profiles ?? [];
  const activeId = payload?.active_profile_id ?? null;
  const queryError = isLoading || !isError ? null : String(error instanceof Error ? error.message : error);

  const handleCreate = useCallback(
    async (input: UpsertProfileInput) => {
      setMutationError(null);
      await upsertProfile(input);
      await invalidate();
    },
    [invalidate],
  );

  const handleUpdate = useCallback(
    async (input: UpsertProfileInput) => {
      setMutationError(null);
      await upsertProfile(input);
      await invalidate();
    },
    [invalidate],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      setMutationError(null);
      await deleteProfile(id);
      await invalidate();
    },
    [invalidate],
  );

  const handleSetActive = useCallback(
    async (id: string) => {
      setMutationError(null);
      await setActiveProfile(id);
      await invalidate();
    },
    [invalidate],
  );

  return (
    <div className="forge-settings-panel-stack">
      {/* ── Toolbar ── */}
      <div className="forge-settings-readonly-panel">
        <div className="forge-memory-toolbar">
          <span className="forge-memory-fact-time" style={{ flex: 1 }}>
            {profiles.length} 个资料
          </span>
          {isFetching && (
            <Loader2 className="size-3.5 animate-spin forge-memory-spinner" />
          )}
          <button
            type="button"
            className="forge-memory-toolbar-btn"
            onClick={() => refetch()}
            aria-label="刷新"
          >
            <RefreshCw className="size-3.5" />
          </button>
          <button
            type="button"
            className="forge-memory-toolbar-btn forge-memory-toolbar-btn--primary"
            onClick={() => setCreating(true)}
            aria-label="新建资料"
          >
            <Plus className="size-3.5" />
          </button>
        </div>
      </div>

      {/* ── Create form ── */}
      {creating && (
        <div className="forge-settings-readonly-panel">
          <ProfileForm
            initial={{
              id: "",
              name: "",
              default_provider: null,
              default_model: null,
              default_workspace: null,
              api_key_overrides: null,
              created_at_ms: 0,
              updated_at_ms: 0,
            }}
            submitLabel="创建"
            onSave={async (input) => {
              await handleCreate(input);
              setCreating(false);
            }}
            onCancel={() => setCreating(false)}
          />
        </div>
      )}

      {/* ── Loading ── */}
      {isLoading && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-empty" role="status">
            <Loader2 className="size-4 animate-spin" />
            <span>加载中…</span>
          </div>
        </div>
      )}

      {/* ── Error ── */}
      {queryError && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-settings-error" role="alert">
            <AlertCircle className="size-3.5" />
            <span>{queryError}</span>
          </div>
        </div>
      )}

      {mutationError && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-settings-error" role="alert">
            <AlertCircle className="size-3.5" />
            <span>{mutationError}</span>
          </div>
        </div>
      )}

      {/* ── Empty (shouldn't happen: seed guarantees default) ── */}
      {!isLoading && !isError && profiles.length === 0 && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-empty">
            <UserRound className="size-4" />
            <span>暂无资料</span>
          </div>
        </div>
      )}

      {/* ── Profile list ── */}
      {profiles.length > 0 && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-list">
            {profiles.map((p) => (
              <ProfileRow
                key={p.id}
                profile={p}
                isActive={p.id === activeId}
                onSetActive={handleSetActive}
                onDelete={handleDelete}
                onUpdate={handleUpdate}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function formatMutationError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "操作失败";
}
