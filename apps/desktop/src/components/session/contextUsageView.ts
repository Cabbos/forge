import type { ContextUsageState } from "@/lib/protocol";

export interface ComposerContextUsageView {
  label: string;
  title: string;
  compactButton: {
    ariaLabel: string;
    disabled: boolean;
    state?: "compacting";
    title: string;
  };
}

interface BuildComposerContextUsageViewOptions {
  fallbackContextWindowTokens?: number | null;
  isCompacting: boolean;
  isStreaming: boolean;
  usage?: ContextUsageState | null;
}

export function buildComposerContextUsageView({
  fallbackContextWindowTokens,
  isCompacting,
  isStreaming,
  usage,
}: BuildComposerContextUsageViewOptions): ComposerContextUsageView {
  const usedTokens = usage?.usedTokens ?? null;
  const contextWindowTokens = usage?.contextWindowTokens ?? fallbackContextWindowTokens ?? null;
  const label = formatComposerContextUsage(usedTokens, contextWindowTokens, {
    isCompacting,
    isStreaming,
    usage,
  });
  const title = formatComposerContextUsageTitle(usage, contextWindowTokens);
  const compactButton = buildCompactButtonView({
    isCompacting,
    isStreaming,
    usageTitle: title,
  });

  return {
    label,
    title,
    compactButton,
  };
}

function buildCompactButtonView({
  isCompacting,
  isStreaming,
  usageTitle,
}: {
  isCompacting: boolean;
  isStreaming: boolean;
  usageTitle: string;
}): ComposerContextUsageView["compactButton"] {
  if (isCompacting) {
    return {
      ariaLabel: "正在压缩上下文",
      disabled: true,
      state: "compacting",
      title: "正在压缩上下文，模型正在生成摘要",
    };
  }

  if (isStreaming) {
    return {
      ariaLabel: "生成中，暂不能压缩上下文",
      disabled: true,
      title: `生成中，完成后可手动压缩 · ${usageTitle || "压缩当前上下文"}`,
    };
  }

  return {
    ariaLabel: "压缩上下文",
    disabled: false,
    title: usageTitle || "压缩当前上下文",
  };
}

function formatComposerContextUsage(
  usedTokens: number | null | undefined,
  contextWindowTokens: number | null | undefined,
  {
    isCompacting,
    isStreaming,
    usage,
  }: {
    isCompacting: boolean;
    isStreaming: boolean;
    usage?: ContextUsageState | null;
  },
) {
  const hasUsedTokens = usedTokens !== null && usedTokens !== undefined;
  const hasContextWindow = contextWindowTokens !== null
    && contextWindowTokens !== undefined
    && contextWindowTokens > 0;
  const baseLabel = hasUsedTokens && hasContextWindow
    ? `${formatTokenCount(usedTokens)} / ${formatTokenCount(contextWindowTokens)}`
    : "";

  if (isCompacting) return baseLabel ? `压缩中 · ${baseLabel}` : "压缩中";
  if (isStreaming) return baseLabel ? `生成中 · ${baseLabel}` : "生成中";
  if (!baseLabel || !hasUsedTokens || !hasContextWindow) return baseLabel;
  if (usage?.compactedFromTokens && usage.compactedToTokens) return `${baseLabel} · 已压缩`;

  return `${baseLabel} · ${formatContextRemainingLabel(usedTokens, contextWindowTokens)}`;
}

function formatComposerContextUsageTitle(
  usage: ContextUsageState | null | undefined,
  fallbackContextWindowTokens?: number | null,
) {
  const usedTokens = usage?.usedTokens ?? null;
  const contextWindowTokens = usage?.contextWindowTokens ?? fallbackContextWindowTokens ?? null;
  if (
    usedTokens === null
    || usedTokens === undefined
    || contextWindowTokens === null
    || contextWindowTokens === undefined
    || contextWindowTokens <= 0
  ) return "压缩当前上下文";

  const percent = usage?.percentUsed !== null && usage?.percentUsed !== undefined ? ` · ${usage.percentUsed}%` : "";
  const source = usage?.source === "local_estimate" ? " · 压缩后估算" : " · 模型 usage";
  const contextRemaining = formatContextRemainingDistance(usedTokens, contextWindowTokens);
  const autoCompact = formatAutoCompactDistance(usedTokens, contextWindowTokens);
  const compacted = usage?.compactedFromTokens && usage.compactedToTokens
    ? ` · 上次压缩 ${formatTokenCount(usage.compactedFromTokens)} -> ${formatTokenCount(usage.compactedToTokens)}`
    : "";

  return `上下文 ${formatTokenCount(usedTokens)} / ${formatTokenCount(contextWindowTokens)}${percent}${source}${contextRemaining}${autoCompact}${compacted}`;
}

function formatTokenCount(tokens: number) {
  if (tokens >= 1_000_000) return `${Math.round(tokens / 1_000_000)}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}K`;
  return String(tokens);
}

function formatAutoCompactDistance(usedTokens: number, contextWindowTokens: number) {
  const threshold = autoCompactThreshold(contextWindowTokens);
  const remaining = threshold - usedTokens;
  const thresholdLabel = ` · 自动压缩阈值 ${formatTokenCount(threshold)}`;
  if (remaining <= 0) return `${thresholdLabel} · 已达到自动压缩阈值`;
  return `${thresholdLabel} · 距离自动压缩还有约 ${formatTokenCount(remaining)} tokens`;
}

function formatContextRemainingDistance(usedTokens: number, contextWindowTokens: number) {
  return ` · 剩余上下文约 ${formatRemainingTokenCount(contextWindowTokens - usedTokens)} tokens`;
}

function formatContextRemainingLabel(usedTokens: number, contextWindowTokens: number) {
  return `余 ${formatRemainingTokenCount(contextWindowTokens - usedTokens)}`;
}

function formatRemainingTokenCount(tokens: number) {
  const safeTokens = Math.max(0, Math.floor(tokens));
  if (safeTokens >= 1_000_000) return formatOneDecimal(Math.floor(safeTokens / 100_000) / 10, "M");
  if (safeTokens >= 1_000) return formatOneDecimal(Math.floor(safeTokens / 100) / 10, "K");
  return String(safeTokens);
}

function formatOneDecimal(value: number, suffix: string) {
  return `${Number.isInteger(value) ? String(value) : value.toFixed(1)}${suffix}`;
}

function autoCompactThreshold(contextWindowTokens: number) {
  const contextLimit = Math.max(16_000, Math.round(contextWindowTokens));
  const reservedOutput = Math.min(20_000, Math.floor(contextLimit / 4));
  const buffer = Math.min(13_000, Math.floor(contextLimit / 10));
  return Math.max(8_000, contextLimit - reservedOutput - buffer);
}
