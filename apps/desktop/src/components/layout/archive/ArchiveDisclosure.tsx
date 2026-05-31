import { useEffect, useState, type ReactNode } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

export function ArchiveDisclosure({
  children,
  defaultOpen = false,
  meta,
  testId,
  title,
}: {
  children: ReactNode;
  defaultOpen?: boolean;
  meta?: string | null;
  testId: string;
  title: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const Icon = open ? ChevronDown : ChevronRight;

  useEffect(() => {
    if (defaultOpen) setOpen(true);
  }, [defaultOpen]);

  return (
    <section data-testid={testId}>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="forge-disclosure-row"
      >
        <span className="flex min-w-0 items-center gap-2">
          <Icon className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="truncate text-[11px] font-medium text-foreground">{title}</span>
        </span>
        {meta && <span className="shrink-0 text-[10px] text-muted-foreground">{meta}</span>}
      </button>
      {open && <div className="mt-2 space-y-3">{children}</div>}
    </section>
  );
}
