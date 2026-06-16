import { useState, useCallback } from "react";
import {
  Brain,
  Plus,
  Search,
  RefreshCw,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useMemoryFactsQuery } from "@/hooks/queries/useMemoryFactsQuery";
import { useProfilesQuery } from "@/hooks/queries/useProfilesQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import {
  upsertMemoryFact,
  deleteMemoryFact,
} from "@/lib/tauri";
import type { MemoryFact } from "@/lib/tauri";
import { MemoryInlineEditor } from "./MemoryInlineEditor";
import { MemoryFactRow } from "./MemoryFactRow";
import {
  buildMemoryFactUpsertInput,
  resolveActiveMemoryProfile,
  resolveMemoryProfileId,
} from "./memoryProfileView";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function MemoryPanel() {
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const { data: profilePayload } = useProfilesQuery();
  const activeProfile = resolveActiveMemoryProfile(profilePayload);
  const activeProfileId = resolveMemoryProfileId(activeProfile?.id);

  const {
    data: facts,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useMemoryFactsQuery(query || undefined, activeProfileId);

  const invalidate = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.memoryFactsAll });
  }, [queryClient]);

  const handleUpsert = useCallback(
    async (text: string, tags: string[]) => {
      setMutationError(null);
      await upsertMemoryFact(buildMemoryFactUpsertInput({
        text,
        tags,
        activeProfileId,
      }));
      await invalidate();
    },
    [activeProfileId, invalidate],
  );

  const handleUpdate = useCallback(
    async (fact: MemoryFact, text: string, tags: string[]) => {
      setMutationError(null);
      await upsertMemoryFact(buildMemoryFactUpsertInput({
        fact,
        text,
        tags,
        activeProfileId,
      }));
      await invalidate();
    },
    [activeProfileId, invalidate],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      setMutationError(null);
      await deleteMemoryFact(id);
      await invalidate();
    },
    [invalidate],
  );

  const queryError = getQueryErrorMessage(isError ? error : null);

  return (
    <div className="forge-settings-panel-stack">
      {/* ── Search + create bar ── */}
      <div className="forge-settings-readonly-panel">
        <div className="forge-memory-toolbar">
          <div className="forge-memory-search">
            <Search className="size-3.5" />
            <input
              type="text"
              className="forge-memory-search-input"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索记忆…"
            />
            {isFetching && (
              <Loader2 className="size-3.5 animate-spin forge-memory-spinner" />
            )}
          </div>
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
            aria-label="新建"
          >
            <Plus className="size-3.5" />
          </ButtonPrimitive>
        </div>
      </div>

      {/* ── Create form ── */}
      {creating && (
        <div className="forge-settings-readonly-panel">
          <MemoryInlineEditor
            initial={{
              id: "",
              text: "",
              tags: [],
              created_at_ms: 0,
              updated_at_ms: 0,
            }}
            onSave={async (text, tags) => {
              await handleUpsert(text, tags);
              setCreating(false);
            }}
            onCancel={() => setCreating(false)}
          />
        </div>
      )}

      {/* ── Loading ── */}
      {isLoading && !facts && (
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

      {/* ── Empty ── */}
      {facts && facts.length === 0 && !query && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-empty">
            <Brain className="size-4" />
            <span>暂无记忆事实</span>
          </div>
        </div>
      )}

      {facts && facts.length === 0 && query && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-empty">
            <Search className="size-4" />
            <span>未找到匹配项</span>
          </div>
        </div>
      )}

      {/* ── Fact list ── */}
      {facts && facts.length > 0 && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-memory-list">
            {facts.map((fact) => (
              <MemoryFactRow
                key={fact.id}
                fact={fact}
                onDelete={handleDelete}
                onUpdate={handleUpdate}
              />
            ))}
          </div>
          <div className="forge-memory-count">
            <span>{facts.length} 条记忆</span>
            {activeProfile && (
              <span className="forge-memory-profile-scope">{activeProfile.name}</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
