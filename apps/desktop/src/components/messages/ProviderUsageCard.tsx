import { Activity } from "lucide-react";
import type { BlockState } from "@/lib/protocol";

export function ProviderUsageCard({ block }: { block: BlockState }) {
  const metadata = block.metadata ?? {};
  const provider = stringMetadata(metadata, "provider_id") ?? "unknown provider";
  const model = stringMetadata(metadata, "model") ?? "unknown model";
  const source = stringMetadata(metadata, "source");
  const reason = stringMetadata(metadata, "reason");
  const inputTokens = tokenLabel(metadata, "input_tokens", "input_tokens_unknown");
  const outputTokens = tokenLabel(metadata, "output_tokens", "output_tokens_unknown");
  const cost = costLabel(metadata);
  const reasonLabel = providerUsageReasonLabel(reason);

  const items = [
    `提供方 ${provider}`,
    `模型 ${model}`,
    `输入 ${inputTokens}`,
    `输出 ${outputTokens}`,
    `费用 ${cost}`,
    source ? `来源 ${source}` : null,
    reasonLabel,
  ].filter(Boolean);

  return (
    <div
      data-testid="provider-usage-card"
      className="forge-provider-usage-card"
      title={block.content}
      aria-label={`模型用量 ${model}`}
    >
      <Activity aria-hidden="true" className="forge-provider-usage-icon size-3" />
      <span className="forge-provider-usage-title">模型用量</span>
      <span className="forge-provider-usage-items">
        {items.map((item) => (
          <span key={item} className="forge-provider-usage-item">{item}</span>
        ))}
      </span>
    </div>
  );
}

function stringMetadata(metadata: Record<string, unknown>, key: string) {
  const value = metadata[key];
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function tokenLabel(metadata: Record<string, unknown>, tokenKey: string, unknownKey: string) {
  if (metadata[unknownKey] === true) return "unknown";
  const value = metadata[tokenKey];
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "unknown";
}

function costLabel(metadata: Record<string, unknown>) {
  if (metadata.cost_unknown === true) return "unknown";
  const value = metadata.estimated_cost_micros;
  return typeof value === "number" && Number.isFinite(value) ? `${value} micros` : "unknown";
}

function providerUsageReasonLabel(reason: string | null) {
  if (reason === "provider_omitted") return "用量未知";
  if (reason === "pricing_unknown") return "价格未知";
  return null;
}
