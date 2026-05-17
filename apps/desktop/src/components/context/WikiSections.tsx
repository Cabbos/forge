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

const EMPTY_FORGE_WIKI_PROPOSALS: ForgeWikiUpdateProposal[] = [];

export function WikiSections({ sessionId, projectPath }: WikiSectionsProps) {
  const memories = useStore((s) => s.memories);
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
  const busyTokenRef = useRef(0);

  const currentProjectPath = useMemo(() => normalizeProjectPath(projectPath), [projectPath]);
  const currentProjectPathRef = useRef(currentProjectPath);
  const sessionIdRef = useRef(sessionId);

  currentProjectPathRef.current = currentProjectPath;
  sessionIdRef.current = sessionId;

  const isCurrentRequest = useCallback((projectAtStart: string, sessionAtStart: string | null) => {
    return currentProjectPathRef.current === projectAtStart && sessionIdRef.current === sessionAtStart;
  }, []);

  const beginBusy = useCallback((id: string) => {
    const token = busyTokenRef.current + 1;
    busyTokenRef.current = token;
    setBusyId(id);
    return token;
  }, []);

  const clearBusy = useCallback((token: number, id: string) => {
    if (busyTokenRef.current !== token) return;
    setBusyId((current) => (current === id ? null : current));
  }, []);

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

  const visibleForgeWikiProposals = useMemo(
    () =>
      forgeWikiProposals.filter(
        (proposal) =>
          (proposal.status === "pending" ||
            proposal.status === "accepted" ||
            proposal.status === "discarded") &&
          (!currentProjectPath || normalizeProjectPath(proposal.project_path) === currentProjectPath),
      ),
    [currentProjectPath, forgeWikiProposals],
  );

  const pendingForgeWikiProposals = useMemo(
    () => visibleForgeWikiProposals.filter((proposal) => proposal.status === "pending"),
    [visibleForgeWikiProposals],
  );

  const handleInitForgeWiki = useCallback(async () => {
    const projectAtStart = currentProjectPath;
    const sessionAtStart = sessionId;
    if (!projectAtStart) return;

    const operationId = "forge-wiki:init";
    const busyToken = beginBusy(operationId);
    setError("");
    try {
      const nextForgeWikiState = await initForgeWiki(projectAtStart);
      if (!isCurrentRequest(projectAtStart, sessionAtStart)) return;
      setForgeWikiState(nextForgeWikiState);
      await refresh();
    } catch (err) {
      if (isCurrentRequest(projectAtStart, sessionAtStart)) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      clearBusy(busyToken, operationId);
    }
  }, [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, refresh, sessionId]);

  const handleAcceptForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      const projectAtStart = currentProjectPath;
      const sessionAtStart = sessionId;
      const busyToken = beginBusy(proposal.id);
      setError("");
      try {
        const nextProposal = await acceptForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionAtStart ?? undefined,
        );
        if (sessionAtStart) upsertForgeWikiProposal(sessionAtStart, nextProposal);
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          await refresh();
        }
      } catch (err) {
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        clearBusy(busyToken, proposal.id);
      }
    },
    [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, refresh, sessionId, upsertForgeWikiProposal],
  );

  const handleDiscardForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      const projectAtStart = currentProjectPath;
      const sessionAtStart = sessionId;
      const busyToken = beginBusy(proposal.id);
      setError("");
      try {
        const nextProposal = await discardForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionAtStart ?? undefined,
        );
        if (sessionAtStart) upsertForgeWikiProposal(sessionAtStart, nextProposal);
      } catch (err) {
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        clearBusy(busyToken, proposal.id);
      }
    },
    [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, sessionId, upsertForgeWikiProposal],
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
        <div className="forge-surface overflow-hidden">
          {!currentProjectPath ? (
            <EmptyState label="打开项目后可以建立项目记录" />
          ) : !forgeWikiState?.exists ? (
            <div className="space-y-3 px-3 py-5 text-center">
              <EmptyState label="还没有项目记录" compact />
              <button
                type="button"
                onClick={handleInitForgeWiki}
                disabled={busyId === "forge-wiki:init"}
                className="forge-action h-8 text-xs focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
              >
                建立项目记录
              </button>
            </div>
          ) : forgeWikiState.pages.length === 0 ? (
            <EmptyState label="还没有项目记录" />
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
          title="建议更新记录"
          meta={
            pendingForgeWikiProposals.length + candidateMemories.length > 0
              ? `${pendingForgeWikiProposals.length + candidateMemories.length} 条`
              : null
          }
        />
        <p className="-mt-1 mb-2 text-[10px] leading-relaxed text-muted-foreground/70">
          确认后会进入项目记录或已保存背景
        </p>
        <div className="forge-surface overflow-hidden">
          {visibleForgeWikiProposals.length === 0 && candidateMemories.length === 0 ? (
            <EmptyState label="没有待确认的记录更新" />
          ) : (
            <div className="divide-y divide-border">
              {candidateMemories.map((memory) => (
                <MemoryRow
                  key={memory.id}
                  memory={memory}
                  draft={draft?.memoryId === memory.id ? draft : null}
                  busy={busyId === memory.id}
                  intentLabel="建议保存为已保存背景"
                  onDraftChange={setDraft}
                  onEdit={() => startEdit(memory)}
                  onSave={saveDraft}
                  onCancel={() => setDraft(null)}
                  onAccept={() => handleAccept(memory.id)}
                  onPin={() => handlePin(memory.id)}
                  onForget={() => handleForget(memory.id)}
                />
              ))}
              {visibleForgeWikiProposals.map((proposal) => (
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
        <SectionHeader title="已保存背景" meta={projectMemories.length > 0 ? `${projectMemories.length} 条` : null} />
        <div className="forge-surface overflow-hidden">
          {projectMemories.length === 0 ? (
            <EmptyState label="还没有已保存背景" />
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
    <div className="forge-section-head">
      <h3 className="forge-section-title">{title}</h3>
      <div className="flex items-center gap-1.5">
        {meta && <span className="forge-section-meta">{meta}</span>}
        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            disabled={refreshDisabled}
            className="forge-icon-button focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
            title="刷新"
          >
            <RefreshCw className={cn("size-3", loading && "animate-spin")} />
          </button>
        )}
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
          <RowIntentLabel>{proposalStatusLabel(proposal.status)}</RowIntentLabel>
          <div className="truncate text-xs font-medium text-foreground">{proposal.title}</div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {proposal.summary}
          </div>
          <RecordMetaGrid
            rows={[
              ["保存位置", "项目记录"],
              ["项目记录页面", proposal.target_pages.join(", ")],
              ["确认后状态", proposalStatusMeta(proposal.status)],
            ]}
          />
        </div>
        {proposal.status === "pending" && (
          <div className="flex shrink-0 gap-0.5">
            <IconButton title="接受" onClick={onAccept} disabled={busy}>
              <Check className="size-3" />
            </IconButton>
            <IconButton title="丢弃" onClick={onDiscard} disabled={busy}>
              <X className="size-3" />
            </IconButton>
          </div>
        )}
      </div>
    </div>
  );
}

function MemoryRow({
  memory,
  draft,
  busy,
  intentLabel,
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
  intentLabel?: string;
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
          {intentLabel && <RowIntentLabel>{intentLabel}</RowIntentLabel>}
          <div className="truncate text-xs font-medium text-foreground">{memory.title}</div>
          <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {memory.body}
          </div>
          {intentLabel && (
            <RecordMetaGrid
              rows={[
                ["保存位置", "已保存背景"],
                ["建议原因", categoryLabel(memory.category)],
                ["确认后状态", "已确认"],
              ]}
            />
          )}
        </div>
        <div className="flex shrink-0 gap-0.5">
          {onAccept && (
            <IconButton title="接受" onClick={onAccept} disabled={busy}>
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

function RecordMetaGrid({ rows }: { rows: Array<[string, string]> }) {
  return (
    <dl className="mt-2 space-y-1 text-[10px] leading-relaxed text-muted-foreground/75">
      {rows.map(([label, value]) => (
        <div key={label} className="grid grid-cols-[64px_minmax(0,1fr)] gap-2">
          <dt className="text-muted-foreground/55">{label}</dt>
          <dd className="min-w-0 break-words">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function proposalStatusLabel(status: ForgeWikiUpdateProposal["status"]) {
  if (status === "accepted") return "已写入项目记录";
  if (status === "discarded") return "已丢弃";
  return "建议写入项目记录";
}

function proposalStatusMeta(status: ForgeWikiUpdateProposal["status"]) {
  if (status === "accepted") return "已写入";
  if (status === "discarded") return "不再处理";
  return "待确认";
}

function RowIntentLabel({ children }: { children: ReactNode }) {
  return (
    <div className="mb-1 text-[10px] font-medium leading-none text-primary/80">
      {children}
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
      className="forge-icon-button focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
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
