import { useEffect, useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import { ToolActivityDetails } from "@/components/messages/ToolActivityDetails";
import { ToolActivitySummary } from "@/components/messages/ToolActivitySummary";
import { deriveToolActivityView } from "./processActivity";

export function ToolActivityGroup({ blocks }: { blocks: BlockState[] }) {
  const activityView = deriveToolActivityView(blocks);
  const [open, setOpen] = useState(activityView.hasError);

  useEffect(() => {
    if (activityView.hasError) setOpen(true);
  }, [activityView.hasError]);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div data-testid="tool-activity-group" className="forge-tool-activity-group" data-tone={activityView.tone}>
        <ToolActivitySummary
          state={activityView.state}
          isRunning={activityView.isRunning}
          label={activityView.label}
          summaryItems={activityView.summaryItems}
          open={open}
        />
        {open && (
          <CollapsibleContent>
            <ToolActivityDetails blocks={blocks} />
          </CollapsibleContent>
        )}
      </div>
    </Collapsible>
  );
}
