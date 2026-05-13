import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Check, Edit3, Pin, RefreshCw, Trash2, X } from "lucide-react";
import { forgetMemory, listMemories, pinMemory, updateMemory } from "@/lib/tauri";
import type { MemoryCategory, MemoryStatus, SelectedContextMemory, WikiMemory } from "@/lib/protocol";
import { cn } from "@/lib/utils";
import { useStore } from "@/store";

interface WikiSectionsProps {
  sessionId: string | null;
  projectPath: string | null;
}

interface DraftState {
  memoryId: string;
  title: string;
  body: string;
}

const EMPTY_SELECTED_CONTEXT: SelectedContextMemory[] = [];

export function WikiSections({ sessionId, projectPath }: WikiSectionsProps) {
  const memories = useStore((s) => s.memories);
  const selectedContext = useStore((s) =>
    sessionId ? s.selectedContextBySession.get(sessionId) ?? EMPTY_SELECTED_CONTEXT : EMPTY_SELECTED_CONTEXT,
  );
  const setMemories = useStore((s) => s.setMemories);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<DraftState | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const currentProjectPath = useMemo(() => normalizeProjectPath(projectPath), [projectPath]);

  const refresh = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    if (!currentProjectPath) {
      setLoading(false);
      setError("");
      return;
    }

    setLoading(true);
    setError("");
    try {
      const next = await listMemories(undefined, currentProjectPath);
      if (requestIdRef.current === requestId) setMemories(next);
    } catch (err) {
      if (requestIdRef.current === requestId) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (requestIdRef.current === requestId) setLoading(false);
    }
  }, [currentProjectPath, setMemories]);

  useEffect(() => {
    refresh();
    return () => {
      requestIdRef.current += 1;
    };
  }, [refresh]);

  const memoriesById = useMemo(() => {
    const byId = new Map<string, WikiMemory>();
    memories
      .filter((memory) => memoryBelongsToCurrentContext(memory, currentProjectPath))
      .forEach((memory) => byId.set(memory.id, memory));
    return byId;
  }, [currentProjectPath, memories]);

  const projectMemories = useMemo(
    () =>
      memories.filter(
        (memory) =>
          memory.scope === "project" &&
          currentProjectPath !== "" &&
          normalizeProjectPath(memory.project_path) === currentProjectPath &&
          (memory.status === "accepted" || memory.status === "pinned"),
      ),
    [currentProjectPath, memories],
  );

  const startEdit = useCallback((memory: WikiMemory) => {
    setDraft({ memoryId: memory.id, title: memory.title, body: memory.body });
  }, []);

  const saveDraft = useCallback(async () => {
    if (!draft) return;
    const memory = memoriesById.get(draft.memoryId);
    if (!memory) return;

    setBusyId(memory.id);
    setError("");
    try {
      await updateMemory(
        memory.id,
        {
          title: draft.title.trim() || memory.title,
          body: draft.body.trim() || memory.body,
          status: memory.status === "candidate" ? "accepted" : memory.status,
        },
        sessionId ?? undefined,
      );
      setDraft(null);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyId(null);
    }
  }, [draft, memoriesById, refresh, sessionId]);

  const handlePin = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await pinMemory(memoryId, sessionId ?? undefined);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [refresh, sessionId],
  );

  const handleForget = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await forgetMemory(memoryId, sessionId ?? undefined);
        if (draft?.memoryId === memoryId) setDraft(null);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [draft?.memoryId, refresh, sessionId],
  );

  return (
    <>
      <section>
        <SectionHeader
          title="相关背景"
          meta={selectedContext.length > 0 ? `已带入 ${selectedContext.length} 条` : null}
          loading={loading}
          onRefresh={refresh}
          refreshDisabled={loading}
        />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {selectedContext.length === 0 ? (
            <EmptyState label="没有找到相关背景" />
          ) : (
            <div className="divide-y divide-border">
              {selectedContext.map((item) => {
                const memory = memoriesById.get(item.memory_id);
                return (
                  <SelectedMemoryRow
                    key={item.memory_id}
                    item={item}
                    memory={memory}
                    draft={draft?.memoryId === item.memory_id ? draft : null}
                    busy={busyId === item.memory_id}
                    onDraftChange={setDraft}
                    onEdit={memory ? () => startEdit(memory) : undefined}
                    onSave={saveDraft}
                    onCancel={() => setDraft(null)}
                    onPin={memory ? () => handlePin(memory.id) : undefined}
                    onForget={memory ? () => handleForget(memory.id) : undefined}
                  />
                );
              })}
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHeader title="项目 Wiki" meta={projectMemories.length > 0 ? `${projectMemories.length} 条` : null} />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {projectMemories.length === 0 ? (
            <EmptyState label="还没有项目 Wiki" />
          ) : (
            <div className="divide-y divide-border">
              {projectMemories.map((memory) => (
                <ProjectMemoryRow key={memory.id} memory={memory} />
              ))}
            </div>
          )}
        </div>
      </section>

      {error && (
        <div className="rounded-md border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-[11px] leading-relaxed text-destructive">
          {error}
        </div>
      )}
    </>
  );
}

function SectionHeader({
  title,
  meta,
  loading = false,
  onRefresh,
  refreshDisabled = false,
}: {
  title: string;
  meta: string | null;
  loading?: boolean;
  onRefresh?: () => void;
  refreshDisabled?: boolean;
}) {
  return (
    <div className="mb-2 flex items-center justify-between gap-2">
      <h3 className="text-[11px] font-medium text-muted-foreground">{title}</h3>
      <div className="flex items-center gap-1.5">
        {meta && <span className="text-[10px] text-muted-foreground/70">{meta}</span>}
        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            disabled={refreshDisabled}
            className="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
            title="刷新"
          >
            <RefreshCw className={cn("size-3", loading && "animate-spin")} />
          </button>
        )}
      </div>
    </div>
  );
}

function SelectedMemoryRow({
  item,
  memory,
  draft,
  busy,
  onDraftChange,
  onEdit,
  onSave,
  onCancel,
  onPin,
  onForget,
}: {
  item: SelectedContextMemory;
  memory?: WikiMemory;
  draft: DraftState | null;
  busy: boolean;
  onDraftChange: (draft: DraftState | null) => void;
  onEdit?: () => void;
  onSave: () => void;
  onCancel: () => void;
  onPin?: () => void;
  onForget?: () => void;
}) {
  if (draft) {
    return (
      <div className="space-y-2 px-3 py-2.5">
        <input
          value={draft.title}
          onChange={(event) => onDraftChange({ ...draft, title: event.target.value })}
          className="w-full rounded border border-border bg-background/70 px-2 py-1 text-xs text-foreground outline-none focus:border-primary/50"
        />
        <textarea
          value={draft.body}
          onChange={(event) => onDraftChange({ ...draft, body: event.target.value })}
          rows={3}
          className="max-h-24 w-full resize-none rounded border border-border bg-background/70 px-2 py-1 text-[11px] leading-relaxed text-foreground outline-none focus:border-primary/50 break-words"
        />
        <div className="flex justify-end gap-1">
          <IconButton title="取消" onClick={onCancel} disabled={busy}>
            <X className="size-3" />
          </IconButton>
          <IconButton title="保存" onClick={onSave} disabled={busy}>
            <Check className="size-3" />
          </IconButton>
        </div>
      </div>
    );
  }

  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-xs font-medium text-foreground">{item.title}</div>
          <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {item.body}
          </div>
        </div>
        {memory && (
          <div className="flex shrink-0 gap-0.5">
            <IconButton title="编辑" onClick={onEdit} disabled={busy}>
              <Edit3 className="size-3" />
            </IconButton>
            <IconButton title="置顶" onClick={onPin} disabled={busy}>
              <Pin className="size-3" />
            </IconButton>
            <IconButton title="忘记" onClick={onForget} disabled={busy}>
              <Trash2 className="size-3" />
            </IconButton>
          </div>
        )}
      </div>
      <div className="mt-2 flex min-w-0 items-center gap-2 text-[10px] text-muted-foreground/70">
        <span className="shrink-0 rounded bg-secondary px-1.5 py-0.5">{categoryLabel(item.category)}</span>
        <span className="min-w-0 truncate break-words">{item.reason}</span>
      </div>
    </div>
  );
}

function ProjectMemoryRow({ memory }: { memory: WikiMemory }) {
  return (
    <div className="px-3 py-2.5">
      <div className="truncate text-xs font-medium text-foreground">{memory.title}</div>
      <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
        {memory.body}
      </div>
      <div className="mt-2 grid grid-cols-[minmax(0,1fr)_58px_48px] gap-2 text-[10px] text-muted-foreground/70">
        <span className="truncate">{categoryLabel(memory.category)}</span>
        <span className="truncate text-right">{statusLabel(memory.status)}</span>
        <span className="text-right font-mono">{Math.round(memory.confidence * 100)}%</span>
      </div>
    </div>
  );
}

function IconButton({
  title,
  disabled,
  onClick,
  children,
}: {
  title: string;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      disabled={disabled || !onClick}
      className="flex size-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
    >
      {children}
    </button>
  );
}

function normalizeProjectPath(path: string | null): string {
  return (path ?? "").trim().replace(/\/+$/, "");
}

function memoryBelongsToCurrentContext(memory: WikiMemory, currentProjectPath: string): boolean {
  if (memory.scope === "user_profile" && !memory.project_path) return true;
  return currentProjectPath !== "" && normalizeProjectPath(memory.project_path) === currentProjectPath;
}

function EmptyState({ label }: { label: string }) {
  return (
    <div className="px-3 py-6 text-center text-xs text-muted-foreground">
      {label}
    </div>
  );
}

function categoryLabel(category: MemoryCategory) {
  switch (category) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目事实";
    case "decision":
      return "决策";
    case "task_state":
      return "任务状态";
  }
}

function statusLabel(status: MemoryStatus) {
  switch (status) {
    case "candidate":
      return "候选";
    case "accepted":
      return "已确认";
    case "pinned":
      return "已置顶";
    case "forgotten":
      return "已忘记";
    case "archived":
      return "已归档";
  }
}
