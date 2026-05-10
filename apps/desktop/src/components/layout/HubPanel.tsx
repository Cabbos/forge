import { useState } from "react";
import { useStore } from "@/store";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

type Tab = "skills" | "mcp" | "hooks";

export function HubPanel() {
  const [tab, setTab] = useState<Tab>("skills");
  const [search, setSearch] = useState("");
  const sessions = useStore((s) => s.sessions);
  const activeId = useStore((s) => s.activeSessionId);
  const session = activeId ? sessions.get(activeId) : null;

  return (
    <aside className="flex flex-col overflow-hidden bg-sidebar">
      <div className="flex border-b border-sidebar-border sticky top-0 z-10 bg-sidebar">
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
              {t === "skills" ? "4" : t === "mcp" ? "3" : "2"}
            </span>
          </button>
        ))}
      </div>

      <ScrollArea className="flex-1">
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
  );
}

function SkillsContent({ search }: { search: string }) {
  const installed = [
    { id: "code-review", desc: "Multi-axis code review", source: "github", icon: "☖", color: "#5B9BD5" },
    { id: "security-audit", desc: "Vuln detection + threat modeling", source: "local", icon: "⚔", color: "#D47777" },
  ];
  const discoverable: Array<{ id: string; desc: string; source: string; stars: number; icon: string; color: string }> = [
    { id: "huashu-design", desc: "HTML-native hi-fi design + motion", source: "alchaincyf", stars: 120, icon: "◆", color: "#D4A853" },
    { id: "data-viz", desc: "Charts, infographics, dashboards", source: "github", stars: 89, icon: "↕", color: "#4A9E6B" },
    { id: "tdd-agent", desc: "Test-driven development workflow", source: "community", stars: 340, icon: "⎔", color: "#5B9BD5" },
  ];
  const filterFn = <T extends { id: string; desc: string }>(list: T[]) =>
    search ? list.filter(s => s.id.includes(search) || s.desc.includes(search)) : list;

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
              <span className="w-6 h-6 rounded flex items-center justify-center text-xs" style={{ background: `${s.color}18`, color: s.color }}>{s.icon}</span>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-foreground">{s.id}</div>
                <div className="text-[10px] text-muted-foreground truncate">{s.desc}</div>
              </div>
              <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-emerald-500/10 text-emerald-400">on</span>
            </div>
          ))}
        </div>
      </section>
      <section>
        <h5 className="text-[10px] uppercase tracking-widest text-muted-foreground/60 mb-2.5">Discover</h5>
        <div className="flex flex-col gap-1.5">
          {filterFn(discoverable).map((s) => (
            <div key={s.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary border border-transparent hover:border-border">
              <span className="w-6 h-6 rounded flex items-center justify-center text-xs" style={{ background: `${s.color}18`, color: s.color }}>{s.icon}</span>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-foreground">{s.id}</div>
                <div className="text-[10px] text-muted-foreground flex gap-2">
                  <span>{s.source}</span>
                  <span className="text-primary">★ {s.stars}</span>
                </div>
              </div>
              <button className="text-[10px] px-2 py-1 rounded font-medium border border-primary text-primary hover:bg-primary hover:text-primary-foreground transition-colors">
                install
              </button>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function MCPContent({ search }: { search: string }) {
  const servers = [
    { id: "playwright-server", on: true },
    { id: "github-tools", on: true },
    { id: "postgres-explorer", on: false },
  ];
  const filtered = search ? servers.filter(s => s.id.includes(search)) : servers;
  return (
    <section className="flex flex-col gap-0.5">
      {filtered.map((s) => (
        <div key={s.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary">
          <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: s.on ? "#4A9E6B" : "#444" }} />
          <span className="flex-1 text-xs text-foreground font-mono">{s.id}</span>
          <div className="relative w-7 h-4 rounded-full cursor-pointer transition-colors flex-shrink-0"
            style={{ background: s.on ? "#D4A853" : "#333" }}>
            <div className="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all"
              style={{ left: s.on ? "14px" : "2px" }} />
          </div>
        </div>
      ))}
    </section>
  );
}

function HooksContent({ search }: { search: string }) {
  const hooks = [
    { id: "logging", trigger: "pre+post", enabled: true },
    { id: "fs-audit", trigger: "post", enabled: true },
  ];
  const filtered = search ? hooks.filter(h => h.id.includes(search)) : hooks;
  return (
    <section className="flex flex-col gap-1.5">
      {filtered.map((h) => (
        <div key={h.id} className="flex items-center gap-2.5 px-2.5 py-2 rounded-md cursor-pointer transition-colors hover:bg-secondary border border-border">
          <div className="flex-1">
            <div className="text-xs font-medium text-foreground">{h.id}</div>
            <div className="text-[10px] text-muted-foreground font-mono">{h.trigger}</div>
          </div>
          <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-emerald-500/10 text-emerald-400">on</span>
        </div>
      ))}
    </section>
  );
}
