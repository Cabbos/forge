import { useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, GitBranch, Link2, Search } from "lucide-react";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { capabilityIconMeta } from "@/lib/capability-icons";
import { cn } from "@/lib/utils";
import { listCapabilities, toggleCapability, type CapabilityInfo } from "@/lib/tauri";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export type CapabilityTab = "skills" | "mcp" | "hooks";

interface CapabilityManagerProps {
  initialTab?: CapabilityTab;
  className?: string;
}

export function CapabilityManager({ initialTab = "skills", className }: CapabilityManagerProps = {}) {
  const managerRef = useRef<HTMLDivElement>(null);
  const [tab, setTab] = useState<CapabilityTab>(initialTab);
  const [search, setSearch] = useState("");
  const [capabilities, setCapabilities] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    setTab(initialTab);
    setSearch("");
  }, [initialTab]);

  useEffect(() => {
    listCapabilities().then(setCapabilities).catch(console.error);
  }, []);

  const tabCapabilities = useMemo(() => ({
    skills: capabilities.filter((c) => c.kind === "skill" || c.kind === "tool"),
    mcp: capabilities.filter((c) => c.kind === "mcp_server"),
    hooks: capabilities.filter((c) => c.kind === "hook"),
  }), [capabilities]);
  const counts = {
    skills: tabCapabilities.skills.length,
    mcp: tabCapabilities.mcp.length,
    hooks: tabCapabilities.hooks.length,
  };
  const enabledCount = capabilities.filter((capability) => capability.enabled !== false).length;
  const activeList = tabCapabilities[tab];
  const activeMatches = filterCapabilities(activeList, search, tab).length;
  const summaryItems = [
    {
      icon: CheckCircle2,
      label: "已启用",
      value: `${enabledCount}/${capabilities.length || 0}`,
    },
    {
      icon: Link2,
      label: "当前类型",
      value: `${activeList.length} 个${tabLabel(tab)}`,
    },
    {
      icon: GitBranch,
      label: search ? "搜索结果" : "筛选",
      value: search ? `${activeMatches} 个匹配` : "未筛选",
    },
  ];

  useGSAP(() => {
    if (prefersReducedMotion()) return;
    const manager = managerRef.current;
    if (!manager) return;

    const entries = gsap.utils.toArray<HTMLElement>(
      "[data-forge-motion='capability-entry']",
      manager,
    );
    if (entries.length === 0) return;

    gsap.fromTo(
      entries,
      { autoAlpha: 0, y: 5 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        stagger: 0.025,
        clearProps: "transform,opacity,visibility",
        onComplete: () => {
          entries.forEach((entry) => {
            if (!entry.getAttribute("style")) entry.removeAttribute("style");
          });
        },
      },
    );
  }, { scope: managerRef, dependencies: [tab, capabilities.length] });

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await toggleCapability(id, enabled);
      setCapabilities((prev) => prev.map((c) => (c.id === id ? { ...c, enabled } : c)));
    } catch (e) {
      console.error("Toggle failed:", e);
    }
  };

  return (
    <div ref={managerRef} data-testid="capability-manager" className={cn("forge-capability-manager", className)}>
      <div role="tablist" aria-label="能力类型" className="forge-capability-tabs">
        {(["skills", "mcp", "hooks"] as CapabilityTab[]).map((item) => (
          <button
            key={item}
            type="button"
            role="tab"
            aria-selected={tab === item}
            data-state={tab === item ? "active" : "idle"}
            onClick={() => setTab(item)}
            className="forge-capability-tab"
          >
            <span>{tabLabel(item)}</span>
            <span className="forge-capability-tab-count">
              {item === "skills" ? counts.skills : item === "mcp" ? counts.mcp : counts.hooks}
            </span>
          </button>
        ))}
      </div>

      <div data-testid="capability-summary-strip" data-forge-motion="capability-entry" className="forge-capability-summary-strip" aria-label="能力摘要">
        {summaryItems.map((item) => (
          <div key={item.label} className="forge-capability-summary-item">
            <ForgeIcon icon={item.icon} tone={item.label === "已启用" ? "safety" : item.label === "当前类型" ? "context" : "action"} contained={false} />
            <span className="forge-capability-summary-copy">
              <span className="forge-capability-summary-label">{item.label}</span>
              <span className="forge-capability-summary-value">{item.value}</span>
            </span>
          </div>
        ))}
      </div>

      <div data-forge-motion="capability-entry" className="forge-capability-search-wrap">
        <label className="forge-capability-search">
          <Search className="forge-capability-search-icon" />
          <input
            type="text"
            aria-label={`搜索${tabLabel(tab)}`}
            placeholder={`搜索${tabLabel(tab)}...`}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            className="forge-capability-search-input"
          />
        </label>
      </div>

      <div data-forge-motion="capability-entry" className="forge-capability-body">
        {tab === "skills" && <SkillsContent caps={tabCapabilities.skills} search={search} onToggle={handleToggle} />}
        {tab === "mcp" && <MCPContent servers={tabCapabilities.mcp} search={search} onToggle={handleToggle} />}
        {tab === "hooks" && <HooksContent hooks={tabCapabilities.hooks} search={search} onToggle={handleToggle} />}
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

function SkillsContent({
  caps,
  search,
  onToggle,
}: {
  caps: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
}) {
  const skills = filterCapabilities(caps, search, "skills");

  return (
    <div className="forge-capability-sections">
      <section className="forge-capability-section">
        <CapabilitySectionHeader label="已安装" count={caps.length} />
        <div className="forge-capability-list">
          {skills.map((s) => {
            const meta = capabilityIconMeta(s.kind);
            return (
              <div key={s.id} className="forge-capability-row" data-state={s.enabled === false ? "disabled" : "enabled"}>
                <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={s.enabled === false} />
                <div className="forge-capability-copy">
                  <div className="forge-capability-name">{s.name}</div>
                  <div className="forge-capability-description">{s.description}</div>
                </div>
                <CapabilityStatusButton
                  enabled={s.enabled !== false}
                  onClick={() => onToggle(s.id, s.enabled === false)}
                />
              </div>
            );
          })}
          {skills.length === 0 && (
            <div className="forge-capability-empty">
              没有匹配的已安装插件
            </div>
          )}
        </div>
      </section>

      <section className="forge-capability-section">
        <CapabilitySectionHeader label="可安装" />
        <div className="forge-capability-empty">
          暂无内置推荐插件
        </div>
      </section>
    </div>
  );
}

function MCPContent({
  servers,
  search,
  onToggle,
}: {
  servers: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
}) {
  const filtered = filterCapabilities(servers, search, "mcp");

  return (
    <section className="forge-capability-list">
      {filtered.map((s) => {
        const meta = capabilityIconMeta(s.kind);
        return (
          <div key={s.id} className="forge-capability-row" data-state={s.enabled === false ? "disabled" : "enabled"}>
            <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={s.enabled === false} />
            <div className="forge-capability-copy">
              <div className="forge-capability-name forge-capability-name-mono">{s.name}</div>
              <div className="forge-capability-description">{s.source || s.id}</div>
            </div>
            <CapabilitySwitch
              enabled={s.enabled !== false}
              label={`${s.name}${s.enabled !== false ? "已启用" : "已停用"}`}
              onClick={() => onToggle(s.id, s.enabled === false)}
            />
          </div>
        );
      })}
      {filtered.length === 0 && (
        <div className="forge-capability-empty">
          没有匹配的连接
        </div>
      )}
    </section>
  );
}

function HooksContent({
  hooks,
  search,
  onToggle,
}: {
  hooks: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
}) {
  const filtered = filterCapabilities(hooks, search, "hooks");

  return (
    <section className="forge-capability-list">
      {filtered.map((h) => {
        const meta = capabilityIconMeta(h.kind);
        return (
          <div key={h.id} className="forge-capability-row" data-state={h.enabled === false ? "disabled" : "enabled"}>
            <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={h.enabled === false} />
            <div className="forge-capability-copy">
              <div className="forge-capability-name">{h.name}</div>
              <div className="forge-capability-description forge-capability-description-mono">{h.version || h.source}</div>
            </div>
            <CapabilityStatusButton
              enabled={h.enabled !== false}
              onClick={() => onToggle(h.id, h.enabled === false)}
            />
          </div>
        );
      })}
      {filtered.length === 0 && (
        <div className="forge-capability-empty">
          没有匹配的自动化
        </div>
      )}
    </section>
  );
}

function CapabilitySectionHeader({ label, count }: { label: string; count?: number }) {
  return (
    <div className="forge-capability-section-header">
      <h5>{label}</h5>
      {typeof count === "number" && <span className="forge-capability-count">{count} 个</span>}
    </div>
  );
}

function CapabilityStatusButton({ enabled, onClick }: { enabled: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={enabled}
      data-state={enabled ? "enabled" : "disabled"}
      className="forge-capability-toggle"
      onClick={onClick}
    >
      {enabled ? "已启用" : "已停用"}
    </button>
  );
}

function filterCapabilities(list: CapabilityInfo[], search: string, tab: CapabilityTab) {
  const query = search.trim().toLowerCase();
  if (!query) return list;
  return list.filter((capability) => {
    const searchable = tab === "mcp"
      ? [capability.name, capability.id, capability.source]
      : [capability.name, capability.description, capability.id, capability.version, capability.source];
    return searchable.some((value) => String(value ?? "").toLowerCase().includes(query));
  });
}

function CapabilitySwitch({ enabled, label, onClick }: { enabled: boolean; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={enabled}
      data-state={enabled ? "enabled" : "disabled"}
      className="forge-capability-switch"
      onClick={onClick}
    >
      <span className="forge-capability-switch-thumb" />
    </button>
  );
}
