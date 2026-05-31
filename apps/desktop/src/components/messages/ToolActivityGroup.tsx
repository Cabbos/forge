import { useEffect, useRef, useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { ForgeCollapsible, ForgeCollapsibleContent } from "@/components/primitives/collapsible";
import { ToolActivityDetails } from "@/components/messages/ToolActivityDetails";
import { ToolActivitySummary } from "@/components/messages/ToolActivitySummary";
import { deriveToolActivityView } from "./processActivity";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function ToolActivityGroup({ blocks }: { blocks: BlockState[] }) {
  const activityView = deriveToolActivityView(blocks);
  const [open, setOpen] = useState(activityView.hasError);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activityView.hasError) setOpen(true);
  }, [activityView.hasError]);

  useGSAP(() => {
    if (!open || prefersReducedMotion()) return;

    const details = rootRef.current?.querySelector<HTMLElement>("[data-forge-motion='activity-details']");
    if (!details) return;

    gsap.fromTo(
      details,
      { autoAlpha: 0, y: -5 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: rootRef, dependencies: [open] });

  return (
    <ForgeCollapsible open={open} onOpenChange={setOpen}>
      <div ref={rootRef} data-testid="tool-activity-group" className="forge-tool-activity-group" data-tone={activityView.tone}>
        <ToolActivitySummary
          state={activityView.state}
          isRunning={activityView.isRunning}
          label={activityView.label}
          summaryItems={activityView.summaryItems}
          open={open}
        />
        {open && (
          <ForgeCollapsibleContent>
            <ToolActivityDetails blocks={blocks} />
          </ForgeCollapsibleContent>
        )}
      </div>
    </ForgeCollapsible>
  );
}
