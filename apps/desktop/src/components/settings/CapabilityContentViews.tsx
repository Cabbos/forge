import { useState } from "react";
import type { CapabilityInfo, EcosystemItem } from "@/lib/tauri";
import type { CapabilityTab } from "@/components/settings/capabilityTypes";
import { filterCapabilities } from "@/components/settings/CapabilityContentModel";
import { CapabilityDetailDrawer } from "@/components/settings/CapabilityDetailDrawer";
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
  /** Optional ecosystem items for richer status/health display. */
  ecosystemItems?: EcosystemItem[];
}

export function CapabilityContentViews({
  tab,
  capabilities,
  search,
  onToggle,
  ecosystemItems,
}: CapabilityContentViewsProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const ecosystemById = new Map((ecosystemItems ?? []).map((e) => [e.id, e]));
  const selected = selectedId ? ecosystemById.get(selectedId) : null;

  const getEcosystem = (cap: CapabilityInfo) => ecosystemById.get(cap.id);

  return (
    <div data-forge-motion="capability-entry" className="forge-capability-body">
      {tab === "skills" && (
        <SkillsContent
          caps={capabilities.skills}
          search={search}
          onToggle={onToggle}
          getEcosystem={getEcosystem}
          onDetails={setSelectedId}
        />
      )}
      {tab === "mcp" && (
        <MCPContent
          servers={capabilities.mcp}
          search={search}
          onToggle={onToggle}
          getEcosystem={getEcosystem}
          onDetails={setSelectedId}
        />
      )}
      {tab === "hooks" && (
        <HooksContent
          hooks={capabilities.hooks}
          search={search}
          onToggle={onToggle}
          getEcosystem={getEcosystem}
          onDetails={setSelectedId}
        />
      )}

      {selected && (
        <CapabilityDetailDrawer
          open
          onClose={() => setSelectedId(null)}
          id={selected.id}
          name={selected.name}
          description={selected.description}
          kind={selected.kind}
          source={selected.source}
          version={selected.version}
          enabled={selected.enabled}
          status={selected.status}
          statusMessage={selected.statusMessage}
          configurable={selected.configurable}
          configSummary={selected.configSummary}
        />
      )}
    </div>
  );
}

function SkillsContent({
  caps,
  search,
  onToggle,
  getEcosystem,
  onDetails,
}: {
  caps: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
  getEcosystem: (cap: CapabilityInfo) => EcosystemItem | undefined;
  onDetails: (id: string) => void;
}) {
  const skills = filterCapabilities(caps, search, "skills");

  return (
    <div className="forge-capability-sections">
      <section className="forge-capability-section">
        <CapabilitySectionHeader label="已安装" count={caps.length} />
        <div className="forge-capability-list">
          {skills.map((s) => {
            const eco = getEcosystem(s);
            return (
              <CapabilityRow
                key={s.id}
                capability={s}
                description={s.description}
                status={eco?.status}
                statusMessage={eco?.statusMessage}
                configurable={eco?.configurable}
                onDetails={eco ? () => onDetails(s.id) : undefined}
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
  getEcosystem,
  onDetails,
}: {
  servers: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
  getEcosystem: (cap: CapabilityInfo) => EcosystemItem | undefined;
  onDetails: (id: string) => void;
}) {
  const filtered = filterCapabilities(servers, search, "mcp");

  return (
    <section className="forge-capability-list">
      {filtered.map((s) => {
        const eco = getEcosystem(s);
        return (
          <CapabilityRow
            key={s.id}
            capability={s}
            nameClassName="forge-capability-name-mono"
            description={s.source || s.id}
            status={eco?.status}
            statusMessage={eco?.statusMessage}
            configurable={eco?.configurable}
            onDetails={eco ? () => onDetails(s.id) : undefined}
            action={(
              <CapabilitySwitch
                enabled={s.enabled !== false}
                label={`${s.name}${s.enabled !== false ? "已启用" : "已停用"}`}
                onClick={() => onToggle(s.id, s.enabled === false)}
              />
            )}
          />
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
  getEcosystem,
  onDetails,
}: {
  hooks: CapabilityInfo[];
  search: string;
  onToggle: (id: string, enabled: boolean) => void;
  getEcosystem: (cap: CapabilityInfo) => EcosystemItem | undefined;
  onDetails: (id: string) => void;
}) {
  const filtered = filterCapabilities(hooks, search, "hooks");

  return (
    <section className="forge-capability-list">
      {filtered.map((h) => {
        const eco = getEcosystem(h);
        return (
          <CapabilityRow
            key={h.id}
            capability={h}
            description={h.version || h.source}
            descriptionClassName="forge-capability-description-mono"
            status={eco?.status}
            statusMessage={eco?.statusMessage}
            configurable={eco?.configurable}
            onDetails={eco ? () => onDetails(h.id) : undefined}
            action={(
              <CapabilityStatusButton
                enabled={h.enabled !== false}
                onClick={() => onToggle(h.id, h.enabled === false)}
              />
            )}
          />
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
