import { BookOpen, Database, FileText, MessageSquareText } from "lucide-react";
import type { ActiveContextItem } from "@/lib/context-activation";
import { activeContextSummary } from "@/lib/context-activation";
import { cn } from "@/lib/utils";

export function ActiveContextSection({ items }: { items: ActiveContextItem[] }) {
  return (
    <section>
      <div className="forge-section-head">
        <h3 className="forge-section-title">本轮参考</h3>
        <span className="forge-section-meta">{activeContextSummary(items)}</span>
      </div>

      {items.length === 0 ? (
        <div className="forge-empty">
          没有额外参考
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((item) => (
            <ActiveContextRow key={`${item.kind}:${item.id}`} item={item} />
          ))}
        </div>
      )}
    </section>
  );
}

function ActiveContextRow({ item }: { item: ActiveContextItem }) {
  const Icon = item.kind === "forge_wiki_page"
    ? BookOpen
    : item.kind === "mcp_resource"
      ? FileText
      : item.kind === "mcp_prompt"
        ? MessageSquareText
        : Database;

  return (
    <article className="forge-surface px-3 py-2.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-1.5">
            <Icon className="size-3 shrink-0 text-muted-foreground" />
            <span className="truncate text-xs font-medium text-foreground">{item.title}</span>
          </div>
          <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{item.summary}</p>
          <div className="mt-1 truncate text-[10px] text-muted-foreground">
            {item.sourceLabel}{item.sourcePath ? ` · ${item.sourcePath}` : ""}
          </div>
        </div>
        <span
          className={cn(
            "forge-pill",
            item.injected ? "border-primary/30 text-primary" : "border-border text-muted-foreground",
          )}
        >
          {item.injected ? "已参考" : "未使用"}
        </span>
      </div>
    </article>
  );
}
