import { useEffect, useState } from "react";
import { Loader2, Power, PowerOff } from "lucide-react";
import { getServiceStatus, setAutostart, type ServiceStatus } from "@/lib/tauri";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

function serviceBackendLabel(status: ServiceStatus | null) {
  switch (status?.backend) {
    case "launchd":
      return "launchd";
    case "systemd":
      return "systemd user service";
    case "windows-service":
      return "Windows Service";
    default:
      return "platform service";
  }
}

export function GeneralSettings() {
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [toggling, setToggling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const s = await getServiceStatus();
        if (!cancelled) setStatus(s);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => { cancelled = true; };
  }, []);

  const handleToggle = async () => {
    if (!status || toggling) return;
    setToggling(true);
    setError(null);
    try {
      const updated = await setAutostart(!status.installed);
      setStatus(updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setToggling(false);
    }
  };

  if (loading) {
    return (
      <div className="forge-settings-section">
        <div className="forge-settings-section-header">
          <h3 className="forge-settings-section-title">通用</h3>
        </div>
        <div className="flex items-center gap-2 text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          <span>检查服务状态...</span>
        </div>
      </div>
    );
  }

  const installed = status?.installed ?? false;
  const running = status?.running ?? false;
  const supported = status?.supported ?? false;
  const backendLabel = serviceBackendLabel(status);

  return (
    <div className="forge-settings-section">
      <div className="forge-settings-section-header">
        <h3 className="forge-settings-section-title">通用</h3>
        <span className="text-xs text-muted-foreground">
          {status?.message ?? ""}
        </span>
      </div>

      {!supported ? (
        <p className="text-sm text-muted-foreground mt-2">
          当前平台暂不支持 Forge Gateway 后台服务管理。
        </p>
      ) : (
        <div className="mt-3 space-y-3">
          {/* Autostart toggle */}
          <div className="flex items-center justify-between rounded-lg border p-3">
            <div>
              <p className="text-sm font-medium">开机自启</p>
              <p className="text-xs text-muted-foreground">
                通过 {backendLabel} 在登录时自动启动 Forge Gateway 后台服务。
              </p>
              <div className="flex items-center gap-2 mt-1">
                <span
                  className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${
                    installed
                      ? "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400"
                      : "bg-muted text-muted-foreground"
                  }`}
                >
                  {installed ? (
                    <Power className="size-3" />
                  ) : (
                    <PowerOff className="size-3" />
                  )}
                  {installed ? "已安装" : "未安装"}
                </span>
                {installed && (
                  <span
                    className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${
                      running
                        ? "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400"
                        : "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400"
                    }`}
                  >
                    {running ? "运行中" : "已停止"}
                  </span>
                )}
              </div>
            </div>
            <ButtonPrimitive
              type="button"
              disabled={toggling}
              onClick={handleToggle}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:opacity-50 ${
                installed ? "bg-primary" : "bg-muted"
              }`}
              role="switch"
              aria-checked={installed}
            >
              <span
                className={`pointer-events-none block size-5 rounded-full bg-white shadow-lg ring-0 transition-transform ${
                  installed ? "translate-x-5" : "translate-x-0"
                }`}
              />
              {toggling && (
                <Loader2 className="absolute inset-0 m-auto size-4 animate-spin text-white" />
              )}
            </ButtonPrimitive>
          </div>

          {/* Status text */}
          <p className="text-xs text-muted-foreground">{status?.message}</p>

          {error && (
            <div className="rounded-md bg-destructive/10 p-2 text-xs text-destructive">
              {error}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
