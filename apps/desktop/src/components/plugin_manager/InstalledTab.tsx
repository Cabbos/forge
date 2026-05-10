import { useEffect, useState } from "react";
import { listPlugins, type PluginEntry } from "@/lib/tauri";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Loader2, Puzzle } from "lucide-react";

export function InstalledTab() {
  const [plugins, setPlugins] = useState<PluginEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadAll();
  }, []);

  async function loadAll() {
    setLoading(true);
    try {
      const agents = ["claude", "codex", "hermes"];
      const results = await Promise.allSettled(
        agents.map((a) => listPlugins(a))
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

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // Group by agent
  const grouped = new Map<string, PluginEntry[]>();
  for (const p of plugins) {
    const key = p.agent;
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key)!.push(p);
  }

  if (grouped.size === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-muted-foreground gap-2">
        <Puzzle className="size-6 opacity-40" />
        <p className="text-xs text-center">No plugins installed</p>
      </div>
    );
  }

  return (
    <ScrollArea className="flex-1">
      <div className="space-y-3 pr-1">
        {Array.from(grouped.entries()).map(([agent, items]) => (
          <div key={agent}>
            <div className="flex items-center gap-1.5 mb-1.5">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {agent}
              </span>
              <Badge variant="secondary" className="text-[10px] px-1 py-0 h-4">
                {items.length}
              </Badge>
            </div>
            <div className="space-y-1">
              {items.map((p) => (
                <PluginRow key={p.id} plugin={p} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}

function PluginRow({ plugin: p }: { plugin: PluginEntry }) {
  const enabled =
    typeof p.status === "object" && "installed" in p.status
      ? (p.status as { installed: { enabled: boolean } }).installed.enabled
      : true;

  return (
    <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-muted/30 text-xs">
      <div
        className={`size-1.5 rounded-full shrink-0 ${
          enabled ? "bg-emerald-500" : "bg-muted-foreground/40"
        }`}
      />
      <span className="truncate flex-1">{p.name}</span>
      <Badge variant="outline" className="text-[9px] px-1 py-0 h-3.5 shrink-0">
        {p.plugin_type.replace("_", " ")}
      </Badge>
    </div>
  );
}
