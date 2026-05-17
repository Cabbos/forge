import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex justify-end">
      <div
        data-testid="user-message"
        className="forge-user-message"
      >
        {block.content}
      </div>
    </div>
  );
}
