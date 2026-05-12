import { useState, useEffect } from "react";
import { X } from "lucide-react";
import { useStore } from "@/store";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { listCapabilities, toggleCapability, installSkill, type CapabilityInfo } from "@/lib/tauri";

type Tab = "skills" | "mcp" | "hooks";

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
      return "#888";
  }
}

function getIcon(kind: string): string {
  switch (kind) {
    case "skill":
      return "☖";
    case "tool":
      return "⎔";
    case "mcp_server":
      return "◈";
    case "hook":
      return "⚙";
    default:
      return "●";
  }
}

export function HubPanel() {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<Tab>("skills");
  const [search, setSearch] = useState("");
  const [counts, setCounts] = useState({ skills: 0, mcp: 0, hooks: 0 });
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const session = activeId ? sessions.get(activeId) : null;

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

  // Listen for toggle from toolbar or keyboard shortcut
  useEffect(() => {
    const handler = () => setOpen((v) => !v);
    window.addEventListener("toggle-hub", handler);
    return () => window.removeEventListener("toggle-hub", handler);
  }, []);

  if (!open) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/20 z-40"
        onClick={() => setOpen(false)}
      />

      {/* Panel */}
      <aside
        className="fixed top-0 right-0 h-full w-[280px] z-50 flex flex-col overflow-hidden animate-[slide-in-right_0.25s_ease-out]"
        style={{
          background: "rgba(10,10,10,0.88)",
          backdropFilter: "blur(20px)",
          WebkitBackdropFilter: "blur(20px)",
          borderLeft: "1px solid rgba(255,255,255,0.06)",
        }}
      >
        {/* Header with close button */}
        <div className="flex items-center justify-between px-4 py-3 flex-shrink-0">
          <span className="text-xs font-semibold text-foreground">Capabilities</span>
          <button
            onClick={() => setOpen(false)}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="size-4" />
          </button>
        </div>

        {/* Tab bar */}
        <div className="flex border-b border-sidebar-border flex-shrink-0">
          {(["skills", "mcp", "hooks"] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={cn(
                "flex-1 py-3 text-center text-xs font-medium border-b-2 transition-colors uppercase tracking-wider",
                tab === t
                  ? "border-primary text-foreground"
                  : "border-transparent text-muted-foreground/60 hover:text-muted-foreground"
              )}
            >
              {t}
              <span className="ml-1 text-[10px] text-muted-foreground/60">
                {t === "skills" ? counts.skills : t === "mcp" ? counts.mcp : counts.hooks}
              </span>
            </button>
          ))}
        </div>

        {/* Content */}
        <ScrollArea className="min-h-0 flex-1">
          <div className="p-4 flex flex-col gap-4">
            <div className="flex items-center gap-2 rounded-lg px-3 py-2 text-xs bg-background border border-border">
              <span className="text-muted-foreground">⌕</span>
              <input
                type="text" placeholder={`Search ${tab}...`}
                value={search} onChange={(e) => setSearch(e.target.value)}
                className="flex-1 bg-transparent border-none outline-none text-foreground placeholder:text-muted-foreground/60 text-xs"
              />
            </div>

            {tab === "skills" && <SkillsContent search={search} />}
            {tab === "mcp" && <MCPContent search={search} />}
            {tab === "hooks" && <HooksContent search={search} />}

            {session && (
              <div className="pt-4 border-t border-border flex flex-col gap-1">
                <div className="flex justify-between text-xs text-muted-foreground font-mono">
                  <span>{session.model}</span>
                  <span className="text-primary">${session.costUsd.toFixed(2)}</span>
                </div>
              </div>
            )}
          </div>
        </ScrollArea>
      </aside>
    </>
  );
}

function SkillsContent({ search }: { search: string }) {
  const [caps, setCaps] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    listCapabilities().then(setCaps).catch(console.error);
  }, []);

  const [installing, setInstalling] = useState<string | null>(null);

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await toggleCapability(id, enabled);
      setCaps((prev) => prev.map((c) => (c.id === id ? { ...c, enabled } : c)));
    } catch (e) { console.error("Toggle failed:", e); }
  };

  const handleInstall = async (repo: string) => {
    setInstalling(repo);
    try {
      await installSkill(repo);
      // Refresh full capability list after install
      const all = await listCapabilities();
      setCaps(all);
    } catch (e) { console.error("Install failed:", e); }
    setInstalling(null);
  };

  const skills = caps.filter((c) => c.kind === "skill" || c.kind === "tool");
  const installed = skills;

  // Static discoverable skills
  const DISCOVERABLE = [
    { id: "huashu-design", name: "huashu-design", description: "HTML hi-fi prototypes + animations", source: "alchaincyf/huashu-design", kind: "skill" },
    { id: "karpathy", name: "karpathy", description: "4 coding principles for LLM agents", source: "forrestchang/andrej-karpathy-skills", kind: "skill" },
  ];

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
        <div className="flex justify-between items-center mb-2.5">
          <h5 className="text-[10px] uppercase tracking-widest text-muted-foreground/60">Installed</h5>
          <button className="text-[10px] text-primary hover:opacity-80">+ discover</button>
        </div>
        <div className="flex flex-col gap-1.5">
          {filterFn(installed).map((s) => (
            <div key={s.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary border border-border">
              <span className="w-6 h-6 rounded flex items-center justify-center text-xs" style={{ background: `${getColor(s.kind)}18`, color: getColor(s.kind) }}>{getIcon(s.kind)}</span>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-foreground">{s.name}</div>
                <div className="text-[10px] text-muted-foreground truncate">{s.description}</div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleToggle(s.id, s.enabled === false);
                }}
                className={cn(
                  "text-[10px] px-1.5 py-0.5 rounded font-medium",
                  s.enabled !== false
                    ? "bg-emerald-500/10 text-emerald-400"
                    : "bg-muted/20 text-muted-foreground",
                )}
              >
                {s.enabled !== false ? "on" : "off"}
              </button>
            </div>
          ))}
          {filterFn(installed).length === 0 && (
            <div className="rounded-md border border-border px-2.5 py-3 text-[11px] text-muted-foreground">
              没有匹配的已安装能力
            </div>
          )}
        </div>
      </section>
      <section>
        <h5 className="text-[10px] uppercase tracking-widest text-muted-foreground/60 mb-2.5">Discover</h5>
        <div className="flex flex-col gap-1.5">
          {DISCOVERABLE.filter(s => !caps.some(c => c.name === s.name)).map((s) => (
            <div key={s.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary border border-transparent hover:border-border">
              <span className="w-6 h-6 rounded flex items-center justify-center text-xs" style={{ background: `${getColor(s.kind)}18`, color: getColor(s.kind) }}>{getIcon(s.kind)}</span>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-foreground">{s.name}</div>
                <div className="text-[10px] text-muted-foreground flex gap-2">
                  <span>{s.source}</span>
                </div>
              </div>
              <button
                onClick={() => handleInstall(s.source)}
                disabled={installing === s.source}
                className="text-[10px] px-2 py-1 rounded font-medium border border-primary text-primary hover:bg-primary hover:text-primary-foreground transition-colors disabled:opacity-50"
              >
                {installing === s.source ? "..." : "install"}
              </button>
            </div>
          ))}
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
        <div key={s.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary">
          <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: s.enabled !== false ? "#4A9E6B" : "#444" }} />
          <span className="flex-1 text-xs text-foreground font-mono">{s.name}</span>
          <div
            className="relative w-7 h-4 rounded-full cursor-pointer transition-colors flex-shrink-0"
            style={{ background: s.enabled !== false ? "#D4A853" : "#333" }}
            onClick={() => handleToggle(s.id, s.enabled === false)}
          >
            <div
              className="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all"
              style={{ left: s.enabled !== false ? "14px" : "2px" }}
            />
          </div>
        </div>
      ))}
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
        <div key={h.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary border border-border">
          <div className="flex-1">
            <div className="text-xs font-medium text-foreground">{h.name}</div>
            <div className="text-[10px] text-muted-foreground font-mono">{h.version || h.source}</div>
          </div>
          <button
            onClick={() => handleToggle(h.id, h.enabled === false)}
            className={cn(
              "text-[10px] px-1.5 py-0.5 rounded font-medium",
              h.enabled !== false
                ? "bg-emerald-500/10 text-emerald-400"
                : "bg-muted/20 text-muted-foreground",
            )}
          >
            {h.enabled !== false ? "on" : "off"}
          </button>
        </div>
      ))}
    </section>
  );
}
