import { useState, useCallback } from "react";
import {
  UserRound,
  Plus,
  RefreshCw,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useProfilesQuery } from "@/hooks/queries/useProfilesQuery";
import { queryKeys } from "@/hooks/queries/queryKeys";
import {
  upsertProfile,
  deleteProfile,
  setActiveProfile,
  type UpsertProfileInput,
} from "@/lib/tauri";
import { ProfileForm } from "./ProfileForm";
import { ProfileRow } from "./ProfileRow";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

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
          <ButtonPrimitive
            type="button"
            className="forge-memory-toolbar-btn"
            onClick={() => refetch()}
            aria-label="刷新"
          >
            <RefreshCw className="size-3.5" />
          </ButtonPrimitive>
          <ButtonPrimitive
            type="button"
            className="forge-memory-toolbar-btn forge-memory-toolbar-btn--primary"
            onClick={() => setCreating(true)}
            aria-label="新建资料"
          >
            <Plus className="size-3.5" />
          </ButtonPrimitive>
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
