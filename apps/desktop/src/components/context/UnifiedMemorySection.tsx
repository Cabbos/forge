import { useCallback, useMemo, useState } from "react";
import { Archive, Brain, Search, Trash2 } from "lucide-react";
import { ForgeSurface } from "@/components/primitives/surface";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useUnifiedMemoriesQuery } from "@/hooks/queries/useUnifiedMemoriesQuery";
import {
  applyUnifiedMemoryAction,
  unifiedMemoryActionErrorMessage,
  type UnifiedMemoryActionKind,
  type UnifiedMemoryListFilter,
  type UnifiedMemoryRecord,
} from "@/lib/tauri";
import { useQueryClient } from "@tanstack/react-query";
import { EmptyState, IconButton, RowIntentLabel, SectionHeader } from "./WikiSectionChrome";

interface UnifiedMemorySectionProps {
  currentProjectPath: string;
  sessionId: string | null;
}

export function UnifiedMemorySection({
  currentProjectPath,
  sessionId,
}: UnifiedMemorySectionProps) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<UnifiedMemoryListFilter>("current");
  const [error, setError] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const trimmedQuery = query.trim();
  const {
    data: memories = [],
    isFetching,
    isError,
    error: queryError,
    refetch,
  } = useUnifiedMemoriesQuery(sessionId, currentProjectPath, trimmedQuery, filter, Boolean(currentProjectPath));
  const readError = getQueryErrorMessage(isError ? queryError : null);
  const displayError = error || (readError ? `记忆读取失败：${readError}` : "");

  const visible = useMemo(
    () =>
      memories.filter((memory) => {
        if (filter === "archived") return memory.status === "archived";
        return memory.status === "accepted" || memory.status === "pinned";
      }),
    [filter, memories],
  );

  const applyAction = useCallback(
    async (memory: UnifiedMemoryRecord, action: UnifiedMemoryActionKind) => {
      setBusyId(memory.id);
      setError("");
      try {
        await applyUnifiedMemoryAction(
          { memory_id: memory.id, action },
          sessionId ?? undefined,
          currentProjectPath,
      );
      await queryClient.invalidateQueries({ queryKey: queryKeys.unifiedMemoriesAll });
    } catch (err) {
        setError(unifiedMemoryActionErrorMessage(err));
    } finally {
        setBusyId(null);
      }
    },
    [currentProjectPath, queryClient, sessionId],
  );

  return (
    <section>
      <SectionHeader
        title="记忆"
        meta={visible.length > 0 ? `${visible.length} 条` : null}
        loading={isFetching}
        onRefresh={() => refetch()}
        refreshDisabled={isFetching}
      />
      <ForgeSurface className="overflow-hidden">
        <div className="border-b border-border px-3 py-2">
          <div className="flex items-center gap-2">
            <label className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-border bg-background/70 px-2 text-[11px] text-muted-foreground focus-within:border-primary/40">
              <Search className="size-3.5 shrink-0" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索记忆、经验、背景"
                className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
              />
            </label>
            <div className="flex h-8 shrink-0 rounded-md border border-border bg-background/70 p-0.5">
              {(["current", "archived"] as const).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  onClick={() => setFilter(mode)}
                  className={`h-7 px-2 text-[11px] ${
                    filter === mode
                      ? "rounded bg-primary/10 text-primary"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {mode === "current" ? "现用" : "归档"}
                </button>
              ))}
            </div>
          </div>
        </div>
        {!currentProjectPath ? (
          <EmptyState label="打开项目后可以查看记忆" />
        ) : visible.length === 0 ? (
          <EmptyState label={trimmedQuery ? "没有匹配记忆" : filter === "archived" ? "还没有归档记忆" : "还没有记忆"} />
        ) : (
          <div className="divide-y divide-border">
            {visible.map((memory) => (
              <UnifiedMemoryRow
                key={memory.id}
                memory={memory}
                busy={busyId === memory.id}
                onApplyAction={(action) => applyAction(memory, action)}
              />
            ))}
          </div>
        )}
      </ForgeSurface>
      {displayError && (
        <div className="mt-2 rounded-md border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-[11px] leading-relaxed text-destructive">
          {displayError}
        </div>
      )}
    </section>
  );
}

function UnifiedMemoryRow({
  memory,
  busy,
  onApplyAction,
}: {
  memory: UnifiedMemoryRecord;
  busy: boolean;
  onApplyAction: (action: UnifiedMemoryActionKind) => void;
}) {
  const action = rowAction(memory);
  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Brain className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <RowIntentLabel>{sourceLabel(memory.source)}</RowIntentLabel>
            <span className="truncate text-[10px] text-muted-foreground/70">
              {kindLabel(memory.kind)}
            </span>
            <span className="ml-auto shrink-0 font-mono text-[10px] text-muted-foreground/60">
              {Math.round(memory.confidence * 100)}%
            </span>
          </div>
          <div className="mt-1 truncate text-xs font-medium text-foreground">{memory.title}</div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {memory.body}
          </div>
          <div className="mt-2 flex flex-wrap gap-1 text-[10px] text-muted-foreground/60">
            <span>{statusLabel(memory.status)}</span>
            {memory.source_session_id && <span className="font-mono">{memory.source_session_id}</span>}
            {memory.profile_id && <span>{memory.profile_id}</span>}
          </div>
        </div>
        {action && (
          <div className="shrink-0">
            <IconButton title={action.label} onClick={() => onApplyAction(action.action)} disabled={busy}>
              <action.icon className="size-3" />
            </IconButton>
          </div>
        )}
      </div>
    </div>
  );
}

function rowAction(memory: UnifiedMemoryRecord) {
  if (memory.status === "archived" || memory.status === "forgotten") return null;
  if (memory.source === "memory_fact") {
    return { action: "forget" as const, label: "忘记记忆", icon: Trash2 };
  }
  return { action: "archive" as const, label: "归档记忆", icon: Archive };
}

function sourceLabel(source: UnifiedMemoryRecord["source"]) {
  switch (source) {
    case "wiki_memory":
      return "背景";
    case "memory_fact":
      return "事实";
    case "continuity_experience":
      return "经验";
  }
}

function kindLabel(kind: UnifiedMemoryRecord["kind"]) {
  switch (kind) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目事实";
    case "decision":
      return "决策";
    case "task_state":
      return "进度";
    case "lesson":
      return "经验";
    case "bug_pattern":
      return "Bug 模式";
    case "workflow":
      return "流程";
  }
}

function statusLabel(status: UnifiedMemoryRecord["status"]) {
  switch (status) {
    case "candidate":
      return "候选";
    case "accepted":
      return "已接受";
    case "pinned":
      return "已置顶";
    case "forgotten":
      return "已忘记";
    case "archived":
      return "已归档";
  }
}
