export function PendingBlock() {
  return (
    <div
      data-testid="pending-block"
      className="inline-flex items-center gap-2 py-1 text-xs select-none"
      style={{ color: "var(--muted-foreground)" }}
    >
      <span data-testid="pending-dots" className="flex gap-1">
        <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor" }} />
        <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor", animationDelay: "0.18s" }} />
        <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor", animationDelay: "0.36s" }} />
      </span>
      <span>正在组织回答</span>
    </div>
  );
}
