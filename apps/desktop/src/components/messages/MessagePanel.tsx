import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

type MessagePanelTone = "default" | "warning" | "danger";

const toneStyles: Record<MessagePanelTone, { border: string; background: string }> = {
  default: { border: "rgba(148,163,184,0.18)", background: "rgba(255,255,255,0.012)" },
  warning: { border: "rgba(212,168,83,0.22)", background: "rgba(212,168,83,0.035)" },
  danger: { border: "rgba(212,119,119,0.3)", background: "rgba(212,119,119,0.055)" },
};

export function MessagePanel({
  children,
  className,
  tone = "default",
}: {
  children: ReactNode;
  className?: string;
  tone?: MessagePanelTone;
}) {
  const style = toneStyles[tone];

  return (
    <div
      data-testid="message-panel"
      className={cn("mb-2.5 max-w-[720px] overflow-hidden rounded-md border", className)}
      style={{ borderColor: style.border, background: style.background }}
    >
      {children}
    </div>
  );
}

export function MessagePanelHeader({
  icon,
  title,
  meta,
  actions,
}: {
  icon?: ReactNode;
  title: string;
  meta?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex min-w-0 items-center justify-between gap-2 border-b px-2.5 py-1.5" style={{ borderColor: "rgba(148,163,184,0.14)" }}>
      <div className="flex min-w-0 items-center gap-2">
        {icon ? <div className="shrink-0 opacity-85">{icon}</div> : null}
        <div className="min-w-0">
          <div className="truncate text-xs font-medium text-foreground">{title}</div>
          {meta ? <div className="mt-0.5 truncate text-[10px] text-muted-foreground/75">{meta}</div> : null}
        </div>
      </div>
      {actions ? <div className="shrink-0">{actions}</div> : null}
    </div>
  );
}
