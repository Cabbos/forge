import { BookOpen, Database, MinusCircle } from "lucide-react";
import type { ActiveContextItem } from "@/lib/context-activation";
import { activeContextSummary } from "@/lib/context-activation";
import { cn } from "@/lib/utils";

export function ActiveContextSection({ items }: { items: ActiveContextItem[] }) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">本轮上下文</h3>
        <span className="text-[10px] text-muted-foreground/70">{activeContextSummary(items)}</span>
      </div>

      {items.length === 0 ? (
        <div className="rounded-md border border-border bg-card px-3 py-4 text-center text-xs text-muted-foreground">
          本轮没有带入额外背景
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
  const Icon = item.kind === "forge_wiki_page" ? BookOpen : Database;

  return (
    <article className="rounded-md border border-border bg-card px-3 py-2.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-1.5">
            <Icon className="size-3 shrink-0 text-muted-foreground" />
            <span className="truncate text-xs font-medium text-foreground">{item.title}</span>
          </div>
          <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{item.summary}</p>
        </div>
        <span
          className={cn(
            "shrink-0 rounded border px-1.5 py-0.5 text-[10px]",
            item.injected ? "border-primary/30 text-primary" : "border-border text-muted-foreground",
          )}
        >
          {item.injected ? "已带入" : "未使用"}
        </span>
      </div>
      <dl className="mt-2 space-y-1 text-[10px] leading-relaxed text-muted-foreground/75">
        <div className="grid grid-cols-[52px_minmax(0,1fr)] gap-2">
          <dt className="text-muted-foreground/55">为什么带入</dt>
          <dd className="min-w-0 break-words">{item.reason}</dd>
        </div>
        <div className="grid grid-cols-[52px_minmax(0,1fr)] gap-2">
          <dt className="text-muted-foreground/55">来源</dt>
          <dd className="min-w-0 truncate">
            {item.sourceLabel}{item.sourcePath ? ` · ${item.sourcePath}` : ""}
          </dd>
        </div>
        <div className="grid grid-cols-[52px_minmax(0,1fr)] gap-2">
          <dt className="text-muted-foreground/55">本轮状态</dt>
          <dd>{item.injected ? "已带入" : "未使用"}</dd>
        </div>
      </dl>
      <div className="mt-2 flex justify-end">
        <button
          type="button"
          disabled
          title="后续支持从本轮移除"
          className="inline-flex items-center gap-1 text-[10px] text-muted-foreground/50"
        >
          <MinusCircle className="size-3" />
          本轮移除
        </button>
      </div>
    </article>
  );
}
