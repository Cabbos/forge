import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex justify-end mb-4">
      <div className="max-w-[72%]">
        <div className="text-[9px] uppercase tracking-wider text-muted-foreground/50 mb-1.5 text-right">你的请求</div>
        <div className="rounded-lg border px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap break-words"
          style={{ background: "rgba(212,168,83,0.04)", borderColor: "rgba(212,168,83,0.08)", color: "#ddd" }}>
          {block.content}
        </div>
      </div>
    </div>
  );
}
