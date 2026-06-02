import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { CheckCircle2, Circle, FolderOpen, GitBranch, KeyRound, Play, RefreshCw } from "lucide-react";
import type {
  ReadinessAction,
  StartReadinessRow,
  StartReadinessView as StartReadinessState,
} from "@/lib/start-readiness";
import { cn } from "@/lib/utils";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeIcon } from "@/components/primitives/icon";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import type { ForgeIconTone } from "@/lib/capability-icons";

interface StartReadinessViewProps {
  readiness: StartReadinessState;
  primaryAction: StartReadinessRow | undefined;
  panelState: "ready" | "blocked" | "attention";
  secondaryStatus: string;
  variant: "panel" | "setup-strip";
  showDetails: boolean;
  loading: boolean;
  busyAction: ReadinessAction;
  onRefresh: () => void;
  onRunAction: (action: ReadinessAction) => void;
}

export function StartReadinessView({
  readiness,
  primaryAction,
  panelState,
  secondaryStatus,
  variant,
  showDetails,
  loading,
  busyAction,
  onRefresh,
  onRunAction,
}: StartReadinessViewProps) {
  if (variant === "setup-strip") {
    const setupAction = primaryAction?.tone === "blocked" ? primaryAction : null;
    if (!setupAction?.action || !setupAction.actionLabel) return null;

    return (
      <div data-testid="start-readiness" className="mx-auto w-full max-w-[460px] px-1 py-1">
        <div data-testid="start-readiness-panel" className="forge-readiness-strip" data-state={panelState}>
          <div className="forge-readiness-strip-icon" aria-hidden="true">
            <KeyRound className="size-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium text-foreground">{readiness.title}</div>
            <div className="mt-0.5 truncate text-xs text-muted-foreground">{setupAction.value}</div>
          </div>
          <ForgeActionButton
            disabled={busyAction === setupAction.action}
            onClick={() => onRunAction(setupAction.action)}
            className="justify-center disabled:cursor-default disabled:opacity-70"
          >
            {busyAction === setupAction.action ? "处理中" : setupAction.actionLabel}
          </ForgeActionButton>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="start-readiness" className="mx-auto max-w-[760px] px-1 py-1">
      <div
        data-testid="start-readiness-panel"
        className="forge-readiness-panel"
        data-state={panelState}
        data-details={showDetails ? "true" : "false"}
      >
        <div className="forge-readiness-header">
          <div className="flex min-w-0 items-start gap-3">
            <div className="forge-readiness-orb" aria-hidden="true">
              <CheckCircle2 className="size-4" />
            </div>
            <div className="min-w-0">
              <div className="text-sm font-medium text-foreground">{readiness.title}</div>
              <div className="mt-1 max-w-[34rem] truncate text-xs text-muted-foreground">
                {secondaryStatus || readiness.subtitle || (primaryAction ? primaryAction.value : "描述你想做什么，Forge 会在当前项目里继续。")}
              </div>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {primaryAction?.action && primaryAction.actionLabel && (
              <ForgeActionButton
                disabled={busyAction === primaryAction.action}
                onClick={() => onRunAction(primaryAction.action)}
                className="justify-center disabled:cursor-default disabled:opacity-70"
              >
                {busyAction === primaryAction.action ? "处理中" : primaryAction.actionLabel}
              </ForgeActionButton>
            )}
            <ForgeIconButton
              onClick={onRefresh}
              title="刷新准备状态"
              aria-label="刷新准备状态"
            >
              <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
            </ForgeIconButton>
          </div>
        </div>

        {showDetails && (
          <ReadinessRows
            rows={readiness.rows}
            busyAction={busyAction}
            onRunAction={onRunAction}
          />
        )}
      </div>
    </div>
  );
}

function ReadinessRows({
  rows,
  busyAction,
  onRunAction,
}: {
  rows: StartReadinessRow[];
  busyAction: ReadinessAction;
  onRunAction: (action: ReadinessAction) => void;
}) {
  return (
    <div className="forge-readiness-grid" aria-label="开始前状态">
      {rows.map((row) => {
        const RowIcon = readinessIconFor(row.label);
        return (
          <div key={row.label} data-testid="start-readiness-row" className="forge-readiness-row" data-tone={row.tone}>
            <ForgeIcon icon={RowIcon} tone={readinessIconTone(row.label, row.tone)} contained={false} className="size-3.5" />
            <div className="min-w-0 flex-1">
              <div className="forge-readiness-row-label">{row.label}</div>
              <div className="forge-readiness-row-value">{row.value}</div>
            </div>
            {row.action && row.actionLabel ? (
              <ButtonPrimitive
                type="button"
                disabled={busyAction === row.action}
                onClick={() => onRunAction(row.action)}
                className="forge-readiness-row-action disabled:cursor-default disabled:opacity-70"
              >
                {busyAction === row.action ? "处理中" : row.actionLabel}
              </ButtonPrimitive>
            ) : (
              <span className="forge-readiness-row-state">
                {row.tone === "ready" ? "就绪" : row.tone === "blocked" ? "待处理" : "可选"}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

function readinessIconFor(label: string) {
  if (label === "当前项目") return FolderOpen;
  if (label === "模型密钥") return KeyRound;
  if (label === "预览") return Play;
  if (label === "检查点") return GitBranch;
  return Circle;
}

function readinessIconTone(label: string, tone: string): ForgeIconTone {
  if (tone === "blocked") return "danger";
  if (label === "当前项目") return "context";
  if (label === "模型密钥" || label === "检查点") return "safety";
  if (label === "预览") return "action";
  return tone === "ready" ? "safety" : "neutral";
}
