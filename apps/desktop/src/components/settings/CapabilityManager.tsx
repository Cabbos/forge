import { useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, GitBranch, Link2, Search } from "lucide-react";
import { ForgeIcon } from "@/components/primitives/icon";
import { filterCapabilities } from "@/components/settings/CapabilityContentModel";
import { CapabilityContentViews } from "@/components/settings/CapabilityContentViews";
import { CapabilityTabs } from "@/components/settings/CapabilityTabs";
import { tabLabel, type CapabilityTab } from "@/components/settings/capabilityTypes";
import { cn } from "@/lib/utils";
import { toggleCapability, type CapabilityInfo } from "@/lib/tauri";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useCapabilitiesQuery } from "@/hooks/queries/useCapabilitiesQuery";
import { useEcosystemItemsQuery } from "@/hooks/queries/useEcosystemItemsQuery";
import { useToolInventoryQuery } from "@/hooks/queries/useToolInventoryQuery";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export type { CapabilityTab } from "@/components/settings/capabilityTypes";

interface CapabilityManagerProps {
  initialTab?: CapabilityTab;
  className?: string;
}

export function CapabilityManager({ initialTab = "skills", className }: CapabilityManagerProps = {}) {
  const managerRef = useRef<HTMLDivElement>(null);
  const [tab, setTab] = useState<CapabilityTab>(initialTab);
  const [search, setSearch] = useState("");
  const queryClient = useQueryClient();
  const {
    data: capabilities = [],
    isError: capabilitiesIsError,
    error: capabilitiesError,
  } = useCapabilitiesQuery();
  const {
    data: ecosystemItems = [],
  } = useEcosystemItemsQuery();
  const {
    data: toolInventory = [],
  } = useToolInventoryQuery();
  const queryError = getQueryErrorMessage(capabilitiesIsError ? capabilitiesError : null);

  useEffect(() => {
    setTab(initialTab);
    setSearch("");
  }, [initialTab]);

  const tabCapabilities = useMemo<Record<CapabilityTab, CapabilityInfo[]>>(() => ({
    skills: capabilities.filter((c) => c.kind === "skill" || c.kind === "tool"),
    mcp: capabilities.filter((c) => c.kind === "mcp_server"),
    hooks: capabilities.filter((c) => c.kind === "hook"),
  }), [capabilities]);
  const counts: Record<CapabilityTab, number> = {
    skills: tabCapabilities.skills.length,
    mcp: tabCapabilities.mcp.length,
    hooks: tabCapabilities.hooks.length,
  };
  const enabledCount = capabilities.filter((capability) => capability.enabled !== false).length;
  const activeList = tabCapabilities[tab];
  const activeMatches = filterCapabilities(activeList, search, tab).length;
  const toolEnabledCount = toolInventory.filter((t) => t.enabled).length;
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
    ...(toolInventory.length > 0
      ? [{
          icon: CheckCircle2,
          label: "可用工具",
          value: `${toolEnabledCount}/${toolInventory.length}`,
        }]
      : []),
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
      await queryClient.invalidateQueries({ queryKey: queryKeys.capabilities });
      await queryClient.invalidateQueries({ queryKey: queryKeys.ecosystemItems });
      await queryClient.invalidateQueries({ queryKey: queryKeys.toolInventory });
    } catch (e) {
      console.error("Toggle failed:", e);
    }
  };

  return (
    <div ref={managerRef} data-testid="capability-manager" className={cn("forge-capability-manager", className)}>
      <CapabilityTabs tab={tab} counts={counts} onTabChange={setTab} />

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

      {queryError && (
        <div role="status" className="rounded-md border border-destructive/20 bg-destructive/5 px-3 py-2 text-xs leading-relaxed text-destructive">
          能力列表读取失败：{queryError}
        </div>
      )}

      <CapabilityContentViews
        tab={tab}
        capabilities={tabCapabilities}
        search={search}
        onToggle={handleToggle}
        ecosystemItems={ecosystemItems}
      />
    </div>
  );
}
