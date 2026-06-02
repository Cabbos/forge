import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { CAPABILITY_TABS, tabLabel, type CapabilityTab } from "@/components/settings/capabilityTypes";

interface CapabilityTabsProps {
  tab: CapabilityTab;
  counts: Record<CapabilityTab, number>;
  onTabChange: (tab: CapabilityTab) => void;
}

export function CapabilityTabs({ tab, counts, onTabChange }: CapabilityTabsProps) {
  return (
    <div role="tablist" aria-label="能力类型" className="forge-capability-tabs">
      {CAPABILITY_TABS.map((item) => (
        <ButtonPrimitive
          key={item}
          type="button"
          role="tab"
          aria-selected={tab === item}
          data-state={tab === item ? "active" : "idle"}
          onClick={() => onTabChange(item)}
          className="forge-capability-tab"
        >
          <span>{tabLabel(item)}</span>
          <span className="forge-capability-tab-count">{counts[item]}</span>
        </ButtonPrimitive>
      ))}
    </div>
  );
}
