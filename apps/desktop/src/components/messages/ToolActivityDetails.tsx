import type { BlockState } from "@/lib/protocol";
import { ShellCard } from "@/components/messages/ShellCard";
import { ToolCallCard } from "@/components/messages/ToolCallCard";

export function ToolActivityDetails({ blocks }: { blocks: BlockState[] }) {
  return (
    <div className="forge-tool-activity-list" data-forge-motion="activity-details">
      {blocks.map((block) => {
        if (block.event_type === "shell") {
          return <ShellCard key={block.block_id} block={block} />;
        }
        return <ToolCallCard key={block.block_id} block={block} />;
      })}
    </div>
  );
}
