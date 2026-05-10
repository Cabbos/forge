import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex gap-3">
      {/* Avatar */}
      <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0"
        style={{ background: "#1A1A1A", color: "#888", fontSize: "0.65rem", fontWeight: 700 }}>
        U
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-[10px] uppercase tracking-wider mb-1.5" style={{ color: "#555" }}>You</div>
        <div className="text-sm leading-relaxed whitespace-pre-wrap break-words" style={{ color: "#CCC" }}>
          {block.content}
        </div>
      </div>
    </div>
  );
}
