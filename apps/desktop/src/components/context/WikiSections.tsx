import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Check, Edit3, Pin, RefreshCw, Trash2, X } from "lucide-react";
import {
  acceptForgeWikiUpdateProposal,
  discardForgeWikiUpdateProposal,
  forgetMemory,
  getForgeWikiState,
  initForgeWiki,
  listMemories,
  pinMemory,
  updateMemory,
} from "@/lib/tauri";
import type {
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  MemoryCategory,
  MemoryStatus,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  WikiMemory,
} from "@/lib/protocol";
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
const EMPTY_FORGE_WIKI_CONTEXT: SelectedForgeWikiPage[] = [];
const EMPTY_FORGE_WIKI_PROPOSALS: ForgeWikiUpdateProposal[] = [];

export function WikiSections({ sessionId, projectPath }: WikiSectionsProps) {
  const memories = useStore((s) => s.memories);
  const selectedContext = useStore((s) =>
    sessionId ? s.selectedContextBySession.get(sessionId) ?? EMPTY_SELECTED_CONTEXT : EMPTY_SELECTED_CONTEXT,
  );
  const selectedForgeWikiContext = useStore((s) =>
    sessionId ? s.forgeWikiContextBySession.get(sessionId) ?? EMPTY_FORGE_WIKI_CONTEXT : EMPTY_FORGE_WIKI_CONTEXT,
  );
  const forgeWikiProposals = useStore((s) =>
    sessionId ? s.forgeWikiProposalsBySession.get(sessionId) ?? EMPTY_FORGE_WIKI_PROPOSALS : EMPTY_FORGE_WIKI_PROPOSALS,
  );
  const setMemories = useStore((s) => s.setMemories);
  const upsertForgeWikiProposal = useStore((s) => s.upsertForgeWikiProposal);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<DraftState | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [forgeWikiState, setForgeWikiState] = useState<ForgeWikiState | null>(null);
  const requestIdRef = useRef(0);

  const currentProjectPath = useMemo(() => normalizeProjectPath(projectPath), [projectPath]);

  const refresh = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    if (!currentProjectPath) {
      setLoading(false);
      setError("");
      setForgeWikiState(null);
      return;
    }

    setLoading(true);
    setError("");
    try {
      const [nextMemories, nextForgeWikiState] = await Promise.all([
        listMemories(undefined, currentProjectPath),
        getForgeWikiState(currentProjectPath),
      ]);
      if (requestIdRef.current === requestId) {
        setMemories(nextMemories);
        setForgeWikiState(nextForgeWikiState);
      }
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

  const candidateMemories = useMemo(
    () =>
      memories.filter(
        (memory) =>
          memory.status === "candidate" &&
          memoryBelongsToCurrentContext(memory, currentProjectPath),
      ),
    [currentProjectPath, memories],
  );

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

  const pendingForgeWikiProposals = useMemo(
    () =>
      forgeWikiProposals.filter(
        (proposal) =>
          proposal.status === "pending" &&
          (!currentProjectPath || normalizeProjectPath(proposal.project_path) === currentProjectPath),
      ),
    [currentProjectPath, forgeWikiProposals],
  );

  const handleInitForgeWiki = useCallback(async () => {
    if (!currentProjectPath) return;
    setBusyId("forge-wiki:init");
    setError("");
    try {
      const nextForgeWikiState = await initForgeWiki(currentProjectPath);
      setForgeWikiState(nextForgeWikiState);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyId(null);
    }
  }, [currentProjectPath, refresh]);

  const handleAcceptForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      setBusyId(proposal.id);
      setError("");
      try {
        const nextProposal = await acceptForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionId ?? undefined,
        );
        if (sessionId) upsertForgeWikiProposal(sessionId, nextProposal);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [refresh, sessionId, upsertForgeWikiProposal],
  );

  const handleDiscardForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      setBusyId(proposal.id);
      setError("");
      try {
        const nextProposal = await discardForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionId ?? undefined,
        );
        if (sessionId) upsertForgeWikiProposal(sessionId, nextProposal);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [sessionId, upsertForgeWikiProposal],
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

  const handleAccept = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await updateMemory(memoryId, { status: "accepted" }, sessionId ?? undefined);
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
          title="项目记录"
          meta={forgeWikiState?.exists ? `${forgeWikiState.pages.length} 页` : null}
          loading={loading}
          onRefresh={refresh}
          refreshDisabled={loading}
        />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {!currentProjectPath ? (
            <EmptyState label="打开项目后可以建立项目 Wiki" />
          ) : !forgeWikiState?.exists ? (
            <div className="space-y-3 px-3 py-5 text-center">
              <EmptyState label="还没有项目 Wiki" compact />
              <button
                type="button"
                onClick={handleInitForgeWiki}
                disabled={busyId === "forge-wiki:init"}
                className="rounded border border-border bg-secondary px-2.5 py-1.5 text-xs text-foreground transition-colors hover:bg-secondary/80 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
              >
                建立项目 Wiki
              </button>
            </div>
          ) : forgeWikiState.pages.length === 0 ? (
            <EmptyState label="还没有项目 Wiki" />
          ) : (
            <div className="divide-y divide-border">
              {forgeWikiState.pages.map((page) => (
                <ForgeWikiPageRow key={page.id} page={page} />
              ))}
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHeader
          title="本轮带入"
          meta={
            selectedForgeWikiContext.length + selectedContext.length > 0
              ? `已带入 ${selectedForgeWikiContext.length + selectedContext.length} 条`
              : null
          }
        />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {selectedForgeWikiContext.length === 0 && selectedContext.length === 0 ? (
            <EmptyState label="没有找到相关背景" />
          ) : (
            <div className="divide-y divide-border">
              {selectedForgeWikiContext.map((item) => (
                <SelectedForgeWikiRow key={item.page_id} item={item} />
              ))}
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
        <SectionHeader
          title="建议更新项目记录"
          meta={pendingForgeWikiProposals.length > 0 ? `${pendingForgeWikiProposals.length} 条` : null}
        />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {pendingForgeWikiProposals.length === 0 ? (
            <EmptyState label="没有待更新项目记录" />
          ) : (
            <div className="divide-y divide-border">
              {pendingForgeWikiProposals.map((proposal) => (
                <ForgeWikiProposalRow
                  key={proposal.id}
                  proposal={proposal}
                  busy={busyId === proposal.id}
                  onAccept={() => handleAcceptForgeWikiProposal(proposal)}
                  onDiscard={() => handleDiscardForgeWikiProposal(proposal)}
                />
              ))}
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHeader title="待确认" meta={candidateMemories.length > 0 ? `${candidateMemories.length} 条` : null} />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {candidateMemories.length === 0 ? (
            <EmptyState label="没有待确认记忆" />
          ) : (
            <div className="divide-y divide-border">
              {candidateMemories.map((memory) => (
                <MemoryRow
                  key={memory.id}
                  memory={memory}
                  draft={draft?.memoryId === memory.id ? draft : null}
                  busy={busyId === memory.id}
                  onDraftChange={setDraft}
                  onEdit={() => startEdit(memory)}
                  onSave={saveDraft}
                  onCancel={() => setDraft(null)}
                  onAccept={() => handleAccept(memory.id)}
                  onPin={() => handlePin(memory.id)}
                  onForget={() => handleForget(memory.id)}
                />
              ))}
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHeader title="上下文记忆" meta={projectMemories.length > 0 ? `${projectMemories.length} 条` : null} />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          {projectMemories.length === 0 ? (
            <EmptyState label="还没有上下文记忆" />
          ) : (
            <div className="divide-y divide-border">
              {projectMemories.map((memory) => (
                <MemoryRow
                  key={memory.id}
                  memory={memory}
                  draft={draft?.memoryId === memory.id ? draft : null}
                  busy={busyId === memory.id}
                  onDraftChange={setDraft}
                  onEdit={() => startEdit(memory)}
                  onSave={saveDraft}
                  onCancel={() => setDraft(null)}
                  onPin={memory.status === "pinned" ? undefined : () => handlePin(memory.id)}
                  onForget={() => handleForget(memory.id)}
                />
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

function ForgeWikiPageRow({ page }: { page: ForgeWikiState["pages"][number] }) {
  return (
    <div className="px-3 py-2.5">
      <div className="min-w-0">
        <div className="truncate text-xs font-medium text-foreground">{page.title}</div>
        <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground/70">{page.path}</div>
        {page.summary && (
          <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {page.summary}
          </div>
        )}
      </div>
    </div>
  );
}

function SelectedForgeWikiRow({ item }: { item: SelectedForgeWikiPage }) {
  return (
    <div className="px-3 py-2.5">
      <div className="min-w-0">
        <div className="truncate text-xs font-medium text-foreground">{item.title}</div>
        <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
          {item.summary}
        </div>
      </div>
      <div className="mt-2 flex min-w-0 items-center gap-2 text-[10px] text-muted-foreground/70">
        <span className="shrink-0 rounded bg-secondary px-1.5 py-0.5">项目记录</span>
        <span className="min-w-0 truncate break-words">{item.reason}</span>
      </div>
    </div>
  );
}

function ForgeWikiProposalRow({
  proposal,
  busy,
  onAccept,
  onDiscard,
}: {
  proposal: ForgeWikiUpdateProposal;
  busy: boolean;
  onAccept: () => void;
  onDiscard: () => void;
}) {
  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-xs font-medium text-foreground">{proposal.title}</div>
          <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground/70">
            {proposal.target_pages.join(", ")}
          </div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {proposal.summary}
          </div>
        </div>
        <div className="flex shrink-0 gap-0.5">
          <IconButton title="接受更新" onClick={onAccept} disabled={busy}>
            <Check className="size-3" />
          </IconButton>
          <IconButton title="丢弃更新" onClick={onDiscard} disabled={busy}>
            <X className="size-3" />
          </IconButton>
        </div>
      </div>
    </div>
  );
}

function MemoryRow({
  memory,
  draft,
  busy,
  onDraftChange,
  onEdit,
  onSave,
  onCancel,
  onAccept,
  onPin,
  onForget,
}: {
  memory: WikiMemory;
  draft: DraftState | null;
  busy: boolean;
  onDraftChange: (draft: DraftState | null) => void;
  onEdit: () => void;
  onSave: () => void;
  onCancel: () => void;
  onAccept?: () => void;
  onPin?: () => void;
  onForget: () => void;
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
          <div className="truncate text-xs font-medium text-foreground">{memory.title}</div>
          <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {memory.body}
          </div>
        </div>
        <div className="flex shrink-0 gap-0.5">
          {onAccept && (
            <IconButton title="确认记忆" onClick={onAccept} disabled={busy}>
              <Check className="size-3" />
            </IconButton>
          )}
          <IconButton title="编辑" onClick={onEdit} disabled={busy}>
            <Edit3 className="size-3" />
          </IconButton>
          {onPin && (
            <IconButton title="置顶" onClick={onPin} disabled={busy}>
              <Pin className="size-3" />
            </IconButton>
          )}
          <IconButton title="忘记" onClick={onForget} disabled={busy}>
            <Trash2 className="size-3" />
          </IconButton>
        </div>
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

function EmptyState({ label, compact = false }: { label: string; compact?: boolean }) {
  return (
    <div className={cn("px-3 text-center text-xs text-muted-foreground", compact ? "py-0" : "py-6")}>
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
