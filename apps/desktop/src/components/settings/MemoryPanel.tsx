import { useState, useCallback } from "react";
import {
  Brain,
  Plus,
  Trash2,
  Search,
  RefreshCw,
  Pencil,
  Check,
  X,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useMemoryFactsQuery } from "@/hooks/queries/useMemoryFactsQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import {
  upsertMemoryFact,
  deleteMemoryFact,
  type MemoryFact,
} from "@/lib/tauri";

// ── Status helpers ───────────────────────────────────────────────────────────

function formatTimestamp(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

// ── Inline editor sub-component ──────────────────────────────────────────────

function InlineEditor({
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
        <button
          type="button"
          className="forge-memory-action-btn"
          onClick={handleSave}
          disabled={saving || !text.trim()}
          aria-label="保存"
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

// ── Fact row ─────────────────────────────────────────────────────────────────

function FactRow({
  fact,
  onDelete,
  onUpdate,
}: {
  fact: MemoryFact;
  onDelete: (id: string) => Promise<void> | void;
  onUpdate: (id: string, text: string, tags: string[]) => Promise<void> | void;
}) {
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleDelete = useCallback(async () => {
    setDeleteError(null);
    setDeleting(true);
    try {
      await onDelete(fact.id);
    } catch (error) {
      setDeleteError(formatMutationError(error));
    } finally {
      setDeleting(false);
    }
  }, [fact.id, onDelete]);

  if (editing) {
    return (
      <div className="forge-memory-fact" data-editing="true">
        <InlineEditor
          initial={fact}
          onSave={async (text, tags) => {
            await onUpdate(fact.id, text, tags);
            setEditing(false);
          }}
          onCancel={() => setEditing(false)}
        />
      </div>
    );
  }

  return (
    <div className="forge-memory-fact">
      <div className="forge-memory-fact-body">
        <p className="forge-memory-fact-text">{fact.text}</p>
        <div className="forge-memory-fact-meta">
          {fact.tags.length > 0 && (
            <span className="forge-memory-fact-tags">
              {fact.tags.map((t) => (
                <span key={t} className="forge-memory-tag">{t}</span>
              ))}
            </span>
          )}
          <span className="forge-memory-fact-time">
            {formatTimestamp(fact.updated_at_ms)}
          </span>
          {fact.source && (
            <span className="forge-memory-fact-source">{fact.source}</span>
          )}
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

// ── Panel ────────────────────────────────────────────────────────────────────

export function MemoryPanel() {
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const {
    data: facts,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useMemoryFactsQuery(query || undefined);

  const invalidate = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: queryKeys.memoryFactsAll });
  }, [queryClient]);

  const handleUpsert = useCallback(
    async (text: string, tags: string[]) => {
      setMutationError(null);
      await upsertMemoryFact({ text, tags });
      await invalidate();
    },
    [invalidate],
  );

  const handleUpdate = useCallback(
    async (id: string, text: string, tags: string[]) => {
      setMutationError(null);
      await upsertMemoryFact({ id, text, tags });
      await invalidate();
    },
    [invalidate],
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
            aria-label="新建"
          >
            <Plus className="size-3.5" />
          </button>
        </div>
      </div>

      {/* ── Create form ── */}
      {creating && (
        <div className="forge-settings-readonly-panel">
          <InlineEditor
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
              <FactRow
                key={fact.id}
                fact={fact}
                onDelete={handleDelete}
                onUpdate={handleUpdate}
              />
            ))}
          </div>
          <div className="forge-memory-count">
            {facts.length} 条记忆
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
