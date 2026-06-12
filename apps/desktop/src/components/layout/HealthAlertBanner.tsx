import { ShieldAlert, X } from "lucide-react";
import { useStore } from "@/store";

const LEVEL_ICONS: Record<string, typeof ShieldAlert> = {
  info: ShieldAlert,
  warn: ShieldAlert,
  critical: ShieldAlert,
};

const LEVEL_BORDER: Record<string, string> = {
  info: "border-blue-500/30 bg-blue-500/10",
  warn: "border-amber-500/30 bg-amber-500/10",
  critical: "border-red-500/30 bg-red-500/10",
};

export function HealthAlertBanner() {
  const alerts = useStore((s) => s.healthAlerts);
  const dismiss = useStore((s) => s.dismissHealthAlert);

  if (alerts.length === 0) return null;

  return (
    <div
      data-testid="health-alert-banner"
      className="flex flex-col gap-1 px-4 py-2"
    >
      {alerts.map((alert) => {
        const Icon = LEVEL_ICONS[alert.level] ?? ShieldAlert;
        const borderCls = LEVEL_BORDER[alert.level] ?? LEVEL_BORDER.warn;

        return (
          <div
            key={alert.alert_id}
            data-testid={`health-alert-${alert.alert_id}`}
            className={`flex items-start gap-3 rounded-md border px-3 py-2 text-sm ${borderCls}`}
          >
            <Icon className="mt-0.5 h-4 w-4 shrink-0 text-current" />
            <div className="min-w-0 flex-1">
              <p className="font-medium text-foreground">{alert.title}</p>
              <p className="text-muted-foreground">{alert.message}</p>
              {alert.remediation && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {alert.remediation}
                </p>
              )}
            </div>
            <button
              type="button"
              aria-label="Dismiss health alert"
              onClick={() => dismiss(alert.alert_id)}
              className="mt-0.5 shrink-0 rounded-sm text-muted-foreground hover:text-foreground"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
