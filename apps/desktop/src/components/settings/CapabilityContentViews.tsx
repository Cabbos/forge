import type { CapabilityInfo } from "@/lib/tauri";
import type { CapabilityTab } from "@/components/settings/capabilityTypes";
import { filterCapabilities } from "@/components/settings/CapabilityContentModel";
import {
  CapabilityRow,
  CapabilitySectionHeader,
  CapabilityStatusButton,
  CapabilitySwitch,
} from "@/components/settings/CapabilityRows";

interface CapabilityContentViewsProps {
  tab: CapabilityTab;
  capabilities: Record<CapabilityTab, CapabilityInfo[]>;
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
}

export function CapabilityContentViews({
  tab,
  capabilities,
  search,
  onToggle,
}: CapabilityContentViewsProps) {
  return (
    <div data-forge-motion="capability-entry" className="forge-capability-body">
      {tab === "skills" && <SkillsContent caps={capabilities.skills} search={search} onToggle={onToggle} />}
      {tab === "mcp" && <MCPContent servers={capabilities.mcp} search={search} onToggle={onToggle} />}
      {tab === "hooks" && <HooksContent hooks={capabilities.hooks} search={search} onToggle={onToggle} />}
    </div>
  );
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
            return (
              <CapabilityRow
                key={s.id}
                capability={s}
                description={s.description}
                action={(
                  <CapabilityStatusButton
                    enabled={s.enabled !== false}
                    onClick={() => onToggle(s.id, s.enabled === false)}
                  />
                )}
              />
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
      {filtered.map((s) => (
        <CapabilityRow
          key={s.id}
          capability={s}
          nameClassName="forge-capability-name-mono"
          description={s.source || s.id}
          action={(
            <CapabilitySwitch
              enabled={s.enabled !== false}
              label={`${s.name}${s.enabled !== false ? "已启用" : "已停用"}`}
              onClick={() => onToggle(s.id, s.enabled === false)}
            />
          )}
        />
      ))}
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
      {filtered.map((h) => (
        <CapabilityRow
          key={h.id}
          capability={h}
          description={h.version || h.source}
          descriptionClassName="forge-capability-description-mono"
          action={(
            <CapabilityStatusButton
              enabled={h.enabled !== false}
              onClick={() => onToggle(h.id, h.enabled === false)}
            />
          )}
        />
      ))}
      {filtered.length === 0 && (
        <div className="forge-capability-empty">
          没有匹配的自动化
        </div>
      )}
    </section>
  );
}
