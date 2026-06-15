import { useEffect, useMemo, useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, Clock3, Pencil, RotateCcw, Search, Trash2, X } from "lucide-react";
import {
  ForgeDialog,
  ForgeDialogContent,
  ForgeDialogDescription,
  ForgeDialogHeader,
  ForgeDialogTitle,
} from "@/components/primitives/dialog";
import {
  deleteSession,
  getSessionStoreStats,
  renameSessionSnapshot,
  resumeSession,
  searchSessionStore,
  type SessionSnapshotStoreStats,
  type SessionSnapshotSummary,
} from "@/lib/tauri";
import { useStore } from "@/store";

interface HistoryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function HistoryDialog({ open, onOpenChange }: HistoryDialogProps) {
  return (
    <ForgeDialog open={open} onOpenChange={onOpenChange}>
      <ForgeDialogContent className="forge-history-dialog sm:max-w-[780px]">
        <ForgeDialogHeader className="forge-history-header">
          <ForgeDialogTitle className="forge-history-title">
            <Clock3 className="size-4" />
            历史
          </ForgeDialogTitle>
          <ForgeDialogDescription>
            搜索本机保存的会话快照，恢复上下文或删除不需要的记录。
          </ForgeDialogDescription>
        </ForgeDialogHeader>
        <HistoryView onRestored={() => onOpenChange(false)} />
      </ForgeDialogContent>
    </ForgeDialog>
  );
}

export function HistoryView({ onRestored }: { onRestored?: () => void }) {
  const addSession = useStore((state) => state.addSession);
  const removeSession = useStore((state) => state.removeSession);
  const [stats, setStats] = useState<SessionSnapshotStoreStats | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SessionSnapshotSummary[]>([]);
  const [providerFilter, setProviderFilter] = useState("all");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busySessionId, setBusySessionId] = useState<string | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editDraft, setEditDraft] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function loadStats() {
      try {
        const nextStats = await getSessionStoreStats();
        if (!cancelled) setStats(nextStats);
      } catch (err) {
        if (!cancelled) setError(userFacingHistoryError(err, "历史统计加载失败"));
      }
    }

    void loadStats();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function runSearch() {
      setLoading(true);
      setError(null);
      try {
        const nextResults = await searchSessionStore(query.trim());
        if (!cancelled) setResults(nextResults);
      } catch (err) {
        if (!cancelled) {
          setResults([]);
          setError(userFacingHistoryError(err, "历史搜索失败"));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void runSearch();
    return () => {
      cancelled = true;
    };
  }, [query]);

  const summary = useMemo(() => formatHistoryStats(stats), [stats]);
  const providerOptions = useMemo(
    () => buildProviderOptions(stats, results),
    [results, stats],
  );
  const visibleResults = useMemo(
    () =>
      providerFilter === "all"
        ? results
        : results.filter((snapshot) => snapshot.provider === providerFilter),
    [providerFilter, results],
  );

  const restoreSnapshot = async (snapshot: SessionSnapshotSummary) => {
    setBusySessionId(snapshot.session_id);
    setError(null);
    try {
      const restored = await resumeSession(snapshot.session_id);
      addSession(
        restored.session_id,
        restored.provider ?? snapshot.provider,
        restored.model ?? snapshot.model,
        snapshot.working_dir,
      );
      onRestored?.();
    } catch (err) {
      setError(userFacingHistoryError(err, "恢复会话失败"));
    } finally {
      setBusySessionId(null);
    }
  };

  const startRenameSnapshot = (snapshot: SessionSnapshotSummary) => {
    setError(null);
    setEditingSessionId(snapshot.session_id);
    setEditDraft(snapshot.summary || snapshot.session_id);
  };

  const cancelRenameSnapshot = () => {
    setEditingSessionId(null);
    setEditDraft("");
  };

  const saveRenameSnapshot = async (snapshot: SessionSnapshotSummary) => {
    const nextSummary = editDraft.trim();
    if (!nextSummary) {
      setError("会话名称不能为空");
      return;
    }

    setBusySessionId(snapshot.session_id);
    setError(null);
    try {
      const renamed = await renameSessionSnapshot({
        sessionId: snapshot.session_id,
        summary: nextSummary,
      });
      const updatedSnapshot = renamed
        ? renamed
        : { ...snapshot, summary: nextSummary, updated_at_ms: Date.now() };
      setResults((current) =>
        current.map((item) =>
          item.session_id === snapshot.session_id ? updatedSnapshot : item
        )
      );
      cancelRenameSnapshot();
    } catch (err) {
      setError(userFacingHistoryError(err, "重命名会话失败"));
    } finally {
      setBusySessionId(null);
    }
  };

  const deleteSnapshot = async (snapshot: SessionSnapshotSummary) => {
    setBusySessionId(snapshot.session_id);
    setError(null);
    try {
      await deleteSession(snapshot.session_id);
      removeSession(snapshot.session_id);
      setResults((current) =>
        current.filter((item) => item.session_id !== snapshot.session_id)
      );
      setStats((current) =>
        current
          ? {
              ...current,
              total_snapshots: Math.max(0, current.total_snapshots - 1),
            }
          : current,
      );
    } catch (err) {
      setError(userFacingHistoryError(err, "删除会话失败"));
    } finally {
      setBusySessionId(null);
    }
  };

  return (
    <section className="forge-history-view" aria-label="历史会话">
      <div className="forge-history-toolbar">
        <label className="forge-history-search">
          <Search className="size-3.5" aria-hidden="true" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索摘要、模型、项目路径"
            className="forge-history-search-input"
          />
        </label>
        <label className="forge-history-filter">
          <span>服务筛选</span>
          <select
            value={providerFilter}
            onChange={(event) => setProviderFilter(event.target.value)}
            className="forge-history-filter-select"
          >
            <option value="all">全部服务</option>
            {providerOptions.map((provider) => (
              <option key={provider} value={provider}>
                {provider}
              </option>
            ))}
          </select>
        </label>
        <span className="forge-history-count">{summary}</span>
      </div>

      {error && (
        <div className="forge-history-error" role="alert">
          {error}
        </div>
      )}

      <div className="forge-history-result-list">
        {visibleResults.map((snapshot) => {
          const busy = busySessionId === snapshot.session_id;
          const editing = editingSessionId === snapshot.session_id;
          return (
            <article key={snapshot.session_id} className="forge-history-result">
              <div className="forge-history-result-main">
                {editing ? (
                  <div className="forge-history-rename">
                    <label className="forge-history-rename-field">
                      <span>会话名称</span>
                      <input
                        value={editDraft}
                        onChange={(event) => setEditDraft(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") void saveRenameSnapshot(snapshot);
                          if (event.key === "Escape") cancelRenameSnapshot();
                        }}
                        className="forge-history-rename-input"
                        autoFocus
                      />
                    </label>
                    <div className="forge-history-rename-actions">
                      <ButtonPrimitive
                        type="button"
                        className="forge-history-icon-btn"
                        aria-label={`保存重命名 ${snapshot.session_id}`}
                        disabled={busy}
                        onClick={() => void saveRenameSnapshot(snapshot)}
                      >
                        <Check className="size-3.5" />
                      </ButtonPrimitive>
                      <ButtonPrimitive
                        type="button"
                        className="forge-history-icon-btn"
                        aria-label={`取消重命名 ${snapshot.session_id}`}
                        disabled={busy}
                        onClick={cancelRenameSnapshot}
                      >
                        <X className="size-3.5" />
                      </ButtonPrimitive>
                    </div>
                  </div>
                ) : (
                  <h3 className="forge-history-result-title">
                    {snapshot.summary || snapshot.session_id}
                  </h3>
                )}
                <p className="forge-history-result-meta">
                  {snapshot.provider}/{snapshot.model} · {snapshot.message_count} 条消息 · {formatTime(snapshot.updated_at_ms)}
                </p>
                <p className="forge-history-result-path">{snapshot.working_dir}</p>
              </div>
              <div className="forge-history-result-actions">
                <ButtonPrimitive
                  type="button"
                  className="forge-history-icon-btn"
                  aria-label={`重命名 ${snapshot.session_id}`}
                  disabled={busy || editing}
                  onClick={() => startRenameSnapshot(snapshot)}
                >
                  <Pencil className="size-3.5" />
                </ButtonPrimitive>
                <ButtonPrimitive
                  type="button"
                  className="forge-history-icon-btn"
                  aria-label={`恢复 ${snapshot.session_id}`}
                  disabled={busy}
                  onClick={() => void restoreSnapshot(snapshot)}
                >
                  <RotateCcw className="size-3.5" />
                </ButtonPrimitive>
                <ButtonPrimitive
                  type="button"
                  className="forge-history-icon-btn forge-history-icon-btn--danger"
                  aria-label={`删除 ${snapshot.session_id}`}
                  disabled={busy}
                  onClick={() => void deleteSnapshot(snapshot)}
                >
                  <Trash2 className="size-3.5" />
                </ButtonPrimitive>
              </div>
            </article>
          );
        })}

        {!loading && visibleResults.length === 0 && (
          <p className="forge-history-empty">没有找到匹配的会话快照。</p>
        )}
        {loading && <p className="forge-history-empty">正在搜索历史...</p>}
      </div>
    </section>
  );
}

function formatHistoryStats(stats: SessionSnapshotStoreStats | null) {
  if (!stats) return "正在读取历史";
  const corrupt = stats.corrupted_snapshots > 0 ? ` · ${stats.corrupted_snapshots} 个损坏` : "";
  return `${stats.total_snapshots} 个快照${corrupt}`;
}

function buildProviderOptions(
  stats: SessionSnapshotStoreStats | null,
  results: SessionSnapshotSummary[],
) {
  const providers = new Set<string>();
  for (const provider of Object.keys(stats?.by_provider ?? {})) {
    providers.add(provider);
  }
  for (const result of results) {
    providers.add(result.provider);
  }
  return Array.from(providers).sort((a, b) => a.localeCompare(b));
}

function formatTime(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function userFacingHistoryError(error: unknown, fallback: string) {
  const raw = error instanceof Error ? error.message : String(error);
  return raw ? `${fallback}：${raw}` : fallback;
}
