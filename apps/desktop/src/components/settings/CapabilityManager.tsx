import { useEffect, useState } from "react";
import { Search } from "lucide-react";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { capabilityIconMeta } from "@/lib/capability-icons";
import { cn } from "@/lib/utils";
import { listCapabilities, toggleCapability, type CapabilityInfo } from "@/lib/tauri";

export type CapabilityTab = "skills" | "mcp" | "hooks";

interface CapabilityManagerProps {
  initialTab?: CapabilityTab;
  className?: string;
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
    <div data-testid="capability-manager" className={cn("forge-capability-manager", className)}>
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

      <div className="forge-capability-search-wrap">
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

      <div className="forge-capability-body">
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
    <div className="forge-capability-sections">
      <section className="forge-capability-section">
        <CapabilitySectionHeader label="已安装" count={skills.length} />
        <div className="forge-capability-list">
          {filterFn(skills).map((s) => {
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
                  onClick={() => handleToggle(s.id, s.enabled === false)}
                />
              </div>
            );
          })}
          {filterFn(skills).length === 0 && (
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
              onClick={() => handleToggle(s.id, s.enabled === false)}
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
              onClick={() => handleToggle(h.id, h.enabled === false)}
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
