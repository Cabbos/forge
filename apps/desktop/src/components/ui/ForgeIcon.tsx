import type { LucideIcon } from "lucide-react";
import type { ForgeIconTone } from "@/lib/capability-icons";
import { cn } from "@/lib/utils";

interface ForgeIconProps {
  icon: LucideIcon;
  tone?: ForgeIconTone;
  contained?: boolean;
  selected?: boolean;
  disabled?: boolean;
  className?: string;
}

export function ForgeIcon({
  icon: Icon,
  tone = "neutral",
  contained = true,
  selected = false,
  disabled = false,
  className,
}: ForgeIconProps) {
  return (
    <span
      data-testid={`forge-icon-${tone}`}
      data-tone={tone}
      data-contained={contained ? "true" : "false"}
      data-selected={selected ? "true" : "false"}
      data-disabled={disabled ? "true" : "false"}
      className={cn("forge-icon", className)}
    >
      <Icon className="size-3.5" />
    </span>
  );
}
