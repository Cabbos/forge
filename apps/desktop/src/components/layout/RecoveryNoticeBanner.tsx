import { AlertTriangle, X } from "lucide-react";
import { useStore } from "@/store";

export function RecoveryNoticeBanner() {
  const notices = useStore((s) => s.recoveryNotices);
  const dismiss = useStore((s) => s.dismissRecoveryNotice);

  if (notices.length === 0) return null;

  return (
    <div
      data-testid="recovery-notice-banner"
      className="flex flex-col gap-1 px-4 py-2"
    >
      {notices.map((notice) => (
        <div
          key={notice.notice_id}
          data-testid={`recovery-notice-${notice.notice_id}`}
          className="flex items-start gap-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
          <div className="min-w-0 flex-1">
            <p className="font-medium text-foreground">{notice.title}</p>
            <p className="text-muted-foreground">{notice.message}</p>
          </div>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => dismiss(notice.notice_id)}
            className="mt-0.5 shrink-0 rounded-sm text-muted-foreground hover:text-foreground"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      ))}
    </div>
  );
}
