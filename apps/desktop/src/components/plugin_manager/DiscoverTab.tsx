import { useEffect, useState } from "react";
import { discoverPlugins, installPlugin, type PluginEntry } from "@/lib/tauri";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Loader2, Download, Check } from "lucide-react";
import { cn } from "@/lib/utils";

export function DiscoverTab() {
  const [plugins, setPlugins] = useState<PluginEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadAll();
  }, []);

  async function loadAll() {
    setLoading(true);
    try {
      const agents = ["claude", "codex", "hermes"];
      const results = await Promise.allSettled(
        agents.map((a) => discoverPlugins(a))
      );
      const all: PluginEntry[] = [];
      results.forEach((r) => {
        if (r.status === "fulfilled") all.push(...r.value);
      });
      setPlugins(all);
    } catch {
      // silently fail
    }
    setLoading(false);
  }

  async function handleInstall(plugin: PluginEntry) {
    setInstalling((prev) => new Set(prev).add(plugin.id));
    try {
      await installPlugin(plugin.id, plugin.agent);
      // Refresh list
      await loadAll();
    } catch (e) {
      console.error("Install failed:", e);
    }
    setInstalling((prev) => {
      const next = new Set(prev);
      next.delete(plugin.id);
      return next;
    });
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const isInstalled = (p: PluginEntry) =>
    typeof p.status! === "object" && "installed" in p.status!;

  return (
    <ScrollArea className="flex-1">
      <div className="space-y-1 pr-1">
        {plugins.map((p) => {
          const installed = isInstalled(p);
          const busy = installing.has(p.id);

          return (
            <div
              key={p.id}
              className={cn(
                "flex items-start gap-2 px-2 py-2 rounded-md border border-border/50",
                installed && "bg-muted/20"
              )}
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span className="text-xs font-medium truncate">{p.name}</span>
                  <Badge
                    variant="secondary"
                    className="text-[9px] px-1 py-0 h-3.5 shrink-0"
                  >
                    {p.agent}
                  </Badge>
                </div>
                <p className="text-[10px] text-muted-foreground mt-0.5 line-clamp-2">
                  {p.description}
                </p>
              </div>
              <Button
                size="sm"
                variant={installed ? "outline" : "default"}
                className="h-6 text-[10px] px-2 shrink-0"
                disabled={installed || busy}
                onClick={() => handleInstall(p)}
              >
                {busy ? (
                  <Loader2 className="size-3 animate-spin" />
                ) : installed ? (
                  <Check className="size-3" />
                ) : (
                  <Download className="size-3" />
                )}
                <span className="ml-1">
                  {installed ? "Installed" : "Install"}
                </span>
              </Button>
            </div>
          );
        })}
      </div>
    </ScrollArea>
  );
}
