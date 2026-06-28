import { useEffect, useState } from "react";
import { WifiOff } from "lucide-react";

function browserIsOnline() {
  return typeof navigator === "undefined" ? true : navigator.onLine;
}

export function NetworkStatusBanner() {
  const [online, setOnline] = useState(browserIsOnline);

  useEffect(() => {
    const syncNetworkState = () => setOnline(browserIsOnline());

    window.addEventListener("online", syncNetworkState);
    window.addEventListener("offline", syncNetworkState);
    syncNetworkState();

    return () => {
      window.removeEventListener("online", syncNetworkState);
      window.removeEventListener("offline", syncNetworkState);
    };
  }, []);

  if (online) return null;

  return (
    <div data-testid="network-status-banner" className="flex px-4 py-2" role="status" aria-live="polite">
      <div className="flex w-full items-start gap-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm">
        <WifiOff className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
        <div className="min-w-0 flex-1">
          <p className="font-medium text-foreground">当前处于离线状态</p>
          <p className="text-muted-foreground">网络恢复后，Forge 会继续接收运行时事件。</p>
        </div>
      </div>
    </div>
  );
}
