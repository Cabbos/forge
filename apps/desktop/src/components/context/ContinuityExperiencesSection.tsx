import { useCallback, useEffect, useMemo, useState } from "react";
import { Archive, Check, Pin, Search, Trash2 } from "lucide-react";
import { ForgeSurface } from "@/components/primitives/surface";
import {
  listContinuityExperiences,
  searchContinuityExperiences,
  updateContinuityExperienceStatus,
  type ContinuityExperience,
  type ContinuityExperienceStatus,
} from "@/lib/tauri";
import { EmptyState, IconButton, RowIntentLabel, SectionHeader } from "./WikiSectionChrome";

interface ContinuityExperiencesSectionProps {
  currentProjectPath: string;
  sessionId: string | null;
}

export function ContinuityExperiencesSection({
  currentProjectPath,
  sessionId,
}: ContinuityExperiencesSectionProps) {
  const [query, setQuery] = useState("");
  const [experiences, setExperiences] = useState<ContinuityExperience[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const trimmedQuery = query.trim();

  const loadExperiences = useCallback(async () => {
    if (!currentProjectPath) {
      setExperiences([]);
      setError("");
      return;
    }

    setLoading(true);
    setError("");
    try {
      const nextExperiences = trimmedQuery
        ? await searchContinuityExperiences(trimmedQuery, sessionId ?? undefined, currentProjectPath, 20)
        : await listContinuityExperiences(sessionId ?? undefined, currentProjectPath);
      setExperiences(nextExperiences ?? []);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [currentProjectPath, sessionId, trimmedQuery]);

  useEffect(() => {
    loadExperiences();
  }, [loadExperiences]);

  const visibleExperiences = useMemo(
    () => (experiences ?? []).filter((experience) => experience.project_path === currentProjectPath),
    [currentProjectPath, experiences],
  );

  const updateStatus = useCallback(
    async (experience: ContinuityExperience, status: ContinuityExperienceStatus) => {
      setBusyId(experience.id);
      setError("");
      try {
        await updateContinuityExperienceStatus(
          experience.id,
          status,
          sessionId ?? undefined,
          currentProjectPath,
        );
        await loadExperiences();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [currentProjectPath, loadExperiences, sessionId],
  );

  return (
    <section>
      <SectionHeader
        title="经验回忆"
        meta={visibleExperiences.length > 0 ? `${visibleExperiences.length} 条` : null}
        loading={loading}
        onRefresh={loadExperiences}
        refreshDisabled={loading}
      />
      <ForgeSurface className="overflow-hidden">
        <div className="border-b border-border px-3 py-2">
          <label className="flex h-8 items-center gap-2 rounded-md border border-border bg-background/70 px-2 text-[11px] text-muted-foreground focus-within:border-primary/40">
            <Search className="size-3.5 shrink-0" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索本地经验"
              className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            />
          </label>
          {currentProjectPath && (
            <div className="mt-1.5 truncate font-mono text-[10px] text-muted-foreground/50">
              DB: {currentProjectPath}/.forge/continuity.db
            </div>
          )}
        </div>
        {!currentProjectPath ? (
          <EmptyState label="打开项目后可以查看经验" />
        ) : visibleExperiences.length === 0 ? (
          <EmptyState label={trimmedQuery ? "没有匹配经验" : "还没有经验"} />
        ) : (
          <div className="divide-y divide-border">
            {visibleExperiences.map((experience) => (
              <ContinuityExperienceRow
                key={experience.id}
                experience={experience}
                busy={busyId === experience.id}
                onUpdateStatus={(status) => updateStatus(experience, status)}
              />
            ))}
          </div>
        )}
      </ForgeSurface>
      {error && (
        <div className="mt-2 rounded-md border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-[11px] leading-relaxed text-destructive">
          {error}
        </div>
      )}
    </section>
  );
}

function ContinuityExperienceRow({
  experience,
  busy,
  onUpdateStatus,
}: {
  experience: ContinuityExperience;
  busy: boolean;
  onUpdateStatus: (status: ContinuityExperienceStatus) => void;
}) {
  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <RowIntentLabel>{statusLabel(experience.status)}</RowIntentLabel>
          <div className="truncate text-xs font-medium text-foreground">{experience.title}</div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {experience.body}
          </div>
          <div className="mt-2 grid grid-cols-[minmax(0,1fr)_58px_48px] gap-2 text-[10px] text-muted-foreground/70">
            <span className="truncate">{kindLabel(experience.kind)}</span>
            <span className="truncate text-right">{statusLabel(experience.status)}</span>
            <span className="text-right font-mono">{Math.round(experience.confidence * 100)}%</span>
          </div>
          {experience.source_session_id && (
            <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground/60">
              {experience.source_session_id}
            </div>
          )}
        </div>
        <div className="flex shrink-0 gap-0.5">
          {statusActions(experience.status).map((action) => (
            <IconButton
              key={action.status}
              title={action.label}
              onClick={() => onUpdateStatus(action.status)}
              disabled={busy}
            >
              <action.icon className="size-3" />
            </IconButton>
          ))}
        </div>
      </div>
    </div>
  );
}

function statusActions(status: ContinuityExperienceStatus) {
  if (status === "candidate") {
    return [
      { status: "accepted" as const, label: "接受", icon: Check },
      { status: "pinned" as const, label: "置顶", icon: Pin },
      { status: "forgotten" as const, label: "忘记", icon: Trash2 },
    ];
  }
  if (status === "accepted") {
    return [
      { status: "pinned" as const, label: "置顶", icon: Pin },
      { status: "archived" as const, label: "归档", icon: Archive },
    ];
  }
  if (status === "pinned") {
    return [
      { status: "archived" as const, label: "归档", icon: Archive },
      { status: "forgotten" as const, label: "忘记", icon: Trash2 },
    ];
  }
  return [];
}

function statusLabel(status: ContinuityExperienceStatus) {
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

function kindLabel(kind: ContinuityExperience["kind"]) {
  switch (kind) {
    case "lesson":
      return "经验";
    case "bug_pattern":
      return "Bug 模式";
    case "workflow":
      return "流程";
    case "decision":
      return "决策";
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目事实";
  }
}
