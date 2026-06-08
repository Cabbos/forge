import type { ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { cn } from "@/lib/utils";

export function SectionHeader({
  title,
  meta,
  loading = false,
  onRefresh,
  refreshDisabled = false,
}: {
  title: string;
  meta: string | null;
  loading?: boolean;
  onRefresh?: () => void;
  refreshDisabled?: boolean;
}) {
  return (
    <div className="forge-section-head">
      <h3 className="forge-section-title">{title}</h3>
      <div className="flex items-center gap-1.5">
        {meta && <span className="forge-section-meta">{meta}</span>}
        {onRefresh && (
          <ForgeIconButton
            onClick={onRefresh}
            disabled={refreshDisabled}
            className="focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
            title="刷新"
          >
            <RefreshCw className={cn("size-3", loading && "animate-spin")} />
          </ForgeIconButton>
        )}
      </div>
    </div>
  );
}

export function IconButton({
  title,
  disabled,
  onClick,
  children,
}: {
  title: string;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <ForgeIconButton
      title={title}
      onClick={onClick}
      disabled={disabled || !onClick}
      className="focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
    >
      {children}
    </ForgeIconButton>
  );
}

export function RowIntentLabel({ children }: { children: ReactNode }) {
  return (
    <div className="mb-1 text-[10px] font-medium leading-none text-primary/80">
      {children}
    </div>
  );
}

export function EmptyState({ label, compact = false }: { label: string; compact?: boolean }) {
  return (
    <div className={cn("px-3 text-center text-xs text-muted-foreground", compact ? "py-0" : "py-6")}>
      {label}
    </div>
  );
}
