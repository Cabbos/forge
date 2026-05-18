import { useEffect } from "react";
import { X } from "lucide-react";
import { CapabilityManager, type CapabilityTab } from "@/components/settings/CapabilityManager";

interface CapabilityDrawerProps {
  open: boolean;
  initialTab: CapabilityTab;
  title: string;
  onClose: () => void;
}

export function CapabilityDrawer({ open, initialTab, title, onClose }: CapabilityDrawerProps) {
  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, open]);

  if (!open) return null;

  return (
    <>
      <div className="forge-capability-overlay" onClick={onClose} />
      <aside
        aria-label={title}
        data-testid="capability-drawer-surface"
        className="forge-capability-drawer"
      >
        <div data-testid="capability-drawer-header" className="forge-titlebar forge-capability-drawer-header">
          <span className="text-xs font-semibold text-foreground">{title}</span>
          <button
            type="button"
            aria-label={`关闭${title}`}
            onClick={onClose}
            className="text-muted-foreground transition-colors hover:text-foreground"
            title="关闭"
          >
            <X className="size-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 px-3 pb-3">
          <CapabilityManager
            initialTab={initialTab}
            className="h-full min-h-0 rounded-none border-0 bg-transparent"
          />
        </div>
      </aside>
    </>
  );
}
