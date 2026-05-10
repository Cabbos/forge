import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex justify-end mb-6">
      <div className="max-w-[72%] rounded-2xl rounded-br-lg px-5 py-3 bg-primary text-primary-foreground text-[14px] leading-relaxed whitespace-pre-wrap break-words shadow-sm">
        {block.content}
      </div>
    </div>
  );
}
