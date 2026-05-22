import type { AriaRole, HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

type MessagePanelTone = "default" | "warning" | "danger";

const toneStyles: Record<MessagePanelTone, { border: string; background: string }> = {
  default: { border: "var(--forge-border-subtle)", background: "rgba(255, 255, 255, 0.008)" },
  warning: { border: "rgba(212, 168, 83, 0.22)", background: "rgba(212, 168, 83, 0.035)" },
  danger: { border: "rgba(212, 119, 119, 0.3)", background: "rgba(212, 119, 119, 0.055)" },
};

export function MessagePanel({
  children,
  className,
  tone = "default",
  role,
  ariaLive,
  style: styleProp,
  ...divProps
}: Omit<HTMLAttributes<HTMLDivElement>, "role" | "aria-live"> & {
  children: ReactNode;
  className?: string;
  tone?: MessagePanelTone;
  role?: AriaRole;
  ariaLive?: "off" | "polite" | "assertive";
}) {
  const style = toneStyles[tone];

  return (
    <div
      {...divProps}
      data-testid="message-panel"
      className={cn("forge-message-panel", className)}
      role={role}
      aria-live={ariaLive}
      style={{ ...styleProp, borderColor: style.border, background: style.background }}
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
    <div className="forge-message-panel-header">
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
