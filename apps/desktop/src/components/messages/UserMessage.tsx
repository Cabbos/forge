import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex justify-end">
      <div
        data-testid="user-message"
        className="max-w-[82%] rounded-lg px-4 py-2.5 text-sm leading-6 whitespace-pre-wrap break-words shadow-sm sm:max-w-[72%]"
        style={{ background: "var(--secondary)", color: "#E8EAEE", overflowWrap: "anywhere" }}
      >
        {block.content}
      </div>
    </div>
  );
}
