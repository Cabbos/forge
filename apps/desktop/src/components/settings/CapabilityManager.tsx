import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Circle, Clock3, FileSearch, Puzzle, Search, Server } from "lucide-react";
import { cn } from "@/lib/utils";
import { listCapabilities, toggleCapability, type CapabilityInfo } from "@/lib/tauri";

export type CapabilityTab = "skills" | "mcp" | "hooks";

interface CapabilityManagerProps {
  initialTab?: CapabilityTab;
  className?: string;
}

function getColor(kind: string): string {
  switch (kind) {
    case "skill":
    case "tool":
      return "#5B9BD5";
    case "mcp_server":
      return "#4A9E6B";
    case "hook":
      return "#D47777";
    default:
      return "#AEB4BF";
  }
}

function getIcon(kind: string): ReactNode {
  switch (kind) {
    case "skill":
      return <Puzzle className="size-3.5" />;
    case "tool":
      return <FileSearch className="size-3.5" />;
    case "mcp_server":
      return <Server className="size-3.5" />;
    case "hook":
      return <Clock3 className="size-3.5" />;
    default:
      return <Circle className="size-3.5" />;
  }
}

export function CapabilityManager({ initialTab = "skills", className }: CapabilityManagerProps = {}) {
  const [tab, setTab] = useState<CapabilityTab>(initialTab);
  const [search, setSearch] = useState("");
  const [counts, setCounts] = useState({ skills: 0, mcp: 0, hooks: 0 });

  useEffect(() => {
    setTab(initialTab);
    setSearch("");
  }, [initialTab]);

  useEffect(() => {
    listCapabilities()
      .then((all) => {
        setCounts({
          skills: all.filter((c) => c.kind === "skill" || c.kind === "tool").length,
          mcp: all.filter((c) => c.kind === "mcp_server").length,
          hooks: all.filter((c) => c.kind === "hook").length,
        });
      })
      .catch(console.error);
  }, []);

  return (
    <div className={cn("flex min-h-[420px] flex-col overflow-hidden rounded-md border border-border bg-background", className)}>
      <div role="tablist" aria-label="能力类型" className="flex border-b border-border">
        {(["skills", "mcp", "hooks"] as CapabilityTab[]).map((item) => (
          <button
            key={item}
            role="tab"
            aria-selected={tab === item}
            onClick={() => setTab(item)}
            className={cn(
              "flex-1 border-b-2 py-3 text-center text-xs font-medium transition-colors",
              tab === item
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground/60 hover:text-muted-foreground",
            )}
          >
            {tabLabel(item)}
            <span className="ml-1 text-[10px] text-muted-foreground/60">
              {item === "skills" ? counts.skills : item === "mcp" ? counts.mcp : counts.hooks}
            </span>
          </button>
        ))}
      </div>

      <div className="border-b border-border p-3">
        <div className="forge-control-surface flex items-center gap-2 px-3 py-2 text-xs">
          <Search className="size-3.5 shrink-0 text-muted-foreground" />
          <input
            type="text"
            aria-label={`搜索${tabLabel(tab)}`}
            placeholder={`搜索${tabLabel(tab)}...`}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            className="flex-1 border-none bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground/60"
          />
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {tab === "skills" && <SkillsContent search={search} />}
        {tab === "mcp" && <MCPContent search={search} />}
        {tab === "hooks" && <HooksContent search={search} />}
      </div>
    </div>
  );
}

function tabLabel(tab: CapabilityTab) {
  switch (tab) {
    case "skills":
      return "插件";
    case "mcp":
      return "连接";
    case "hooks":
      return "自动化";
  }
}

function SkillsContent({ search }: { search: string }) {
  const [caps, setCaps] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    listCapabilities().then(setCaps).catch(console.error);
  }, []);

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await toggleCapability(id, enabled);
      setCaps((prev) => prev.map((c) => (c.id === id ? { ...c, enabled } : c)));
    } catch (e) {
      console.error("Toggle failed:", e);
    }
  };

  const skills = caps.filter((c) => c.kind === "skill" || c.kind === "tool");

  const filterFn = (list: CapabilityInfo[]) =>
    search
      ? list.filter(
          (s) =>
            s.name.toLowerCase().includes(search.toLowerCase()) ||
            s.description.toLowerCase().includes(search.toLowerCase()),
        )
      : list;

  return (
    <div className="flex flex-col gap-5">
      <section>
        <div className="mb-2.5 flex items-center justify-between">
          <h5 className="text-[10px] uppercase tracking-widest text-muted-foreground/60">已安装</h5>
          <span className="text-[10px] text-muted-foreground/50">{skills.length} 个</span>
        </div>
        <div className="flex flex-col gap-1.5">
          {filterFn(skills).map((s) => (
            <div key={s.id} className="forge-list-row flex items-center gap-2.5 px-2.5 py-2">
              <span className="flex h-6 w-6 items-center justify-center rounded text-xs" style={{ background: `${getColor(s.kind)}18`, color: getColor(s.kind) }}>{getIcon(s.kind)}</span>
              <div className="min-w-0 flex-1">
                <div className="text-xs font-medium text-foreground">{s.name}</div>
                <div className="truncate text-[10px] text-muted-foreground">{s.description}</div>
              </div>
              <button
                onClick={() => handleToggle(s.id, s.enabled === false)}
                aria-pressed={s.enabled !== false}
                className={cn(
                  "rounded px-1.5 py-0.5 text-[10px] font-medium",
                  s.enabled !== false
                    ? "bg-emerald-500/10 text-emerald-400"
                    : "bg-muted/20 text-muted-foreground",
                )}
              >
                {s.enabled !== false ? "已启用" : "已停用"}
              </button>
            </div>
          ))}
          {filterFn(skills).length === 0 && (
            <div className="forge-empty px-2.5 py-3 text-[11px]">
              没有匹配的已安装插件
            </div>
          )}
        </div>
      </section>

      <section>
        <h5 className="mb-2.5 text-[10px] uppercase tracking-widest text-muted-foreground/60">可安装</h5>
        <div className="forge-empty px-2.5 py-3 text-[11px]">
          暂无内置推荐插件
        </div>
      </section>
    </div>
  );
}

function MCPContent({ search }: { search: string }) {
  const [servers, setServers] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    listCapabilities()
      .then((all) => setServers(all.filter((c) => c.kind === "mcp_server")))
      .catch(console.error);
  }, []);

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await toggleCapability(id, enabled);
      setServers((prev) => prev.map((s) => (s.id === id ? { ...s, enabled } : s)));
    } catch (e) {
      console.error("Toggle failed:", e);
    }
  };

  const filtered = search
    ? servers.filter(
        (s) =>
          s.name.toLowerCase().includes(search.toLowerCase()) ||
          s.id.toLowerCase().includes(search.toLowerCase()),
      )
    : servers;

  return (
    <section className="flex flex-col gap-0.5">
      {filtered.map((s) => (
        <div key={s.id} className="forge-list-row flex items-center gap-2.5 px-2.5 py-2">
          <span className="h-1.5 w-1.5 flex-shrink-0 rounded-full" style={{ background: s.enabled !== false ? "#4A9E6B" : "#8C93A0" }} />
          <span className="flex-1 font-mono text-xs text-foreground">{s.name}</span>
          <button
            aria-pressed={s.enabled !== false}
            className="relative h-4 w-7 flex-shrink-0 rounded-full transition-colors"
            style={{ background: s.enabled !== false ? "#D4A853" : "var(--secondary)" }}
            onClick={() => handleToggle(s.id, s.enabled === false)}
          >
            <div
              className="absolute top-0.5 h-3 w-3 rounded-full bg-white transition-all"
              style={{ left: s.enabled !== false ? "14px" : "2px" }}
            />
          </button>
        </div>
      ))}
      {filtered.length === 0 && (
        <div className="forge-empty px-2.5 py-3 text-[11px]">
          没有匹配的连接
        </div>
      )}
    </section>
  );
}

function HooksContent({ search }: { search: string }) {
  const [hooks, setHooks] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    listCapabilities()
      .then((all) => setHooks(all.filter((c) => c.kind === "hook")))
      .catch(console.error);
  }, []);

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await toggleCapability(id, enabled);
      setHooks((prev) => prev.map((h) => (h.id === id ? { ...h, enabled } : h)));
    } catch (e) {
      console.error("Toggle failed:", e);
    }
  };

  const filtered = search
    ? hooks.filter(
        (h) =>
          h.name.toLowerCase().includes(search.toLowerCase()) ||
          h.id.toLowerCase().includes(search.toLowerCase()),
      )
    : hooks;

  return (
    <section className="flex flex-col gap-1.5">
      {filtered.map((h) => (
        <div key={h.id} className="forge-list-row flex items-center gap-2.5 px-2.5 py-2">
          <div className="flex-1">
            <div className="text-xs font-medium text-foreground">{h.name}</div>
            <div className="font-mono text-[10px] text-muted-foreground">{h.version || h.source}</div>
          </div>
          <button
            onClick={() => handleToggle(h.id, h.enabled === false)}
            aria-pressed={h.enabled !== false}
            className={cn(
              "rounded px-1.5 py-0.5 text-[10px] font-medium",
              h.enabled !== false
                ? "bg-emerald-500/10 text-emerald-400"
                : "bg-muted/20 text-muted-foreground",
            )}
          >
            {h.enabled !== false ? "已启用" : "已停用"}
          </button>
        </div>
      ))}
      {filtered.length === 0 && (
        <div className="forge-empty px-2.5 py-3 text-[11px]">
          没有匹配的自动化
        </div>
      )}
    </section>
  );
}
