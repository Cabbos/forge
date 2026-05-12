import { useState } from "react";
import { ArrowUpRight, CheckCircle2, ClipboardCheck, Copy, ShieldCheck } from "lucide-react";
import { useStore } from "@/store";

const ACCEPTANCE_PROMPTS = [
  {
    title: "验收结果",
    prompt: "请用清楚的产品语言总结刚才做了什么，并列出我现在应该逐项验收的地方。",
  },
  {
    title: "检查风险",
    prompt: "请检查刚才的改动有没有风险、遗漏或需要我确认的地方，并按严重程度排序。",
  },
  {
    title: "继续打磨",
    prompt: "请基于当前结果，继续找一个最影响使用体验的问题并直接优化，最后给我验收提示词。",
  },
];

export function AcceptanceCard() {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [loadedIndex, setLoadedIndex] = useState<number | null>(null);
  const setPendingInput = useStore((s) => s.setPendingInput);

  const copyPrompt = async (prompt: string, index: number) => {
    await navigator.clipboard?.writeText(prompt);
    setCopiedIndex(index);
    window.setTimeout(() => setCopiedIndex(null), 1200);
  };

  const loadPrompt = (prompt: string, index: number) => {
    setPendingInput(prompt);
    setLoadedIndex(index);
    window.setTimeout(() => setLoadedIndex(null), 1200);
  };

  return (
      <div className="mb-5 max-w-[760px] rounded-lg border"
      style={{ background: "var(--card)", borderColor: "var(--border)" }}>
      <div className="flex items-center gap-2 border-b px-3 py-2"
        style={{ borderColor: "var(--border)" }}>
        <ClipboardCheck className="size-4" style={{ color: "#D4A853" }} />
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">验收清单</div>
          <div className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>
            不确定有没有完成时，可以按下面的顺序检查。
          </div>
        </div>
      </div>

      <div className="grid gap-2 px-3 py-3 text-xs sm:grid-cols-3" style={{ color: "#D0D5DD" }}>
        <div className="flex gap-2">
          <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" style={{ color: "#4A9E6B" }} />
          <span>确认结果是否符合目标</span>
        </div>
        <div className="flex gap-2">
          <ShieldCheck className="mt-0.5 size-3.5 shrink-0" style={{ color: "#5B9BD5" }} />
          <span>检查风险和遗漏</span>
        </div>
        <div className="flex gap-2">
          <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" style={{ color: "#D4A853" }} />
          <span>按清单完成验收</span>
        </div>
      </div>

      <div className="space-y-1.5 border-t px-3 py-3" style={{ borderColor: "var(--border)" }}>
        <div className="text-[10px] uppercase tracking-wider" style={{ color: "var(--muted-foreground)" }}>
          下一步提示词
        </div>
        {ACCEPTANCE_PROMPTS.map((item, index) => (
          <div
            key={item.prompt}
            className="grid gap-2 rounded-md border px-3 py-2 text-xs sm:grid-cols-[1fr_auto]"
            style={{ background: "var(--background)", borderColor: "var(--border)", color: "#D0D5DD" }}
          >
            <div className="min-w-0">
              <div className="mb-1 text-[11px] font-medium text-foreground/90">
                {item.title}
              </div>
              <div className="leading-relaxed" style={{ color: "var(--muted-foreground)" }}>
                {item.prompt}
              </div>
            </div>
            <div className="flex items-center gap-1.5 sm:justify-end">
              <button
                type="button"
                onClick={() => loadPrompt(item.prompt, index)}
                className="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] transition-colors"
                style={{ background: "#D4A853", color: "#111216" }}
              >
                <ArrowUpRight className="size-3" />
                {loadedIndex === index ? "已放入" : "放入输入框"}
              </button>
              <button
                type="button"
                onClick={() => copyPrompt(item.prompt, index)}
                className="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] transition-colors hover:text-foreground"
                style={{ borderColor: "var(--border)", color: copiedIndex === index ? "#4A9E6B" : "var(--muted-foreground)" }}
              >
                <Copy className="size-3" />
                {copiedIndex === index ? "已复制" : "复制"}
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
