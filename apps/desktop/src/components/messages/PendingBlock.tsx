import { Loader2 } from "lucide-react";

export function PendingBlock() {
  return (
    <div
      data-testid="pending-block"
      className="flex items-center gap-2 py-1 text-xs select-none"
      style={{ color: "var(--muted-foreground)" }}
    >
      <Loader2 className="size-3.5 animate-spin" />
      <span>正在处理...</span>
    </div>
  );
}
