import { useState } from "react";
import { CheckCircle2, ClipboardCheck, Copy, ShieldCheck } from "lucide-react";

const ACCEPTANCE_PROMPTS = [
  "请用小白能懂的话总结刚才做了什么，并指出我应该重点验收哪里。",
  "请检查刚才的改动有没有风险、遗漏或需要我确认的地方。",
  "请帮我验证当前项目是否能正常运行，并列出通过和未验证的部分。",
];

export function AcceptanceCard() {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const copyPrompt = async (prompt: string, index: number) => {
    await navigator.clipboard?.writeText(prompt);
    setCopiedIndex(index);
    window.setTimeout(() => setCopiedIndex(null), 1200);
  };

  return (
    <div className="mb-5 max-w-[760px] rounded-lg border"
      style={{ background: "#0b0b0b", borderColor: "#1b1b1b" }}>
      <div className="flex items-center gap-2 border-b px-3 py-2"
        style={{ borderColor: "#171717" }}>
        <ClipboardCheck className="size-4" style={{ color: "#D4A853" }} />
        <div className="min-w-0">
          <div className="text-sm font-medium" style={{ color: "#ddd" }}>下一步验收</div>
          <div className="text-[11px]" style={{ color: "#777" }}>
            不确定有没有完成时，可以按下面的顺序检查。
          </div>
        </div>
      </div>

      <div className="grid gap-2 px-3 py-3 text-xs sm:grid-cols-3" style={{ color: "#aaa" }}>
        <div className="flex gap-2">
          <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" style={{ color: "#4A9E6B" }} />
          <span>先看结果是否符合你的目标</span>
        </div>
        <div className="flex gap-2">
          <ShieldCheck className="mt-0.5 size-3.5 shrink-0" style={{ color: "#5B9BD5" }} />
          <span>再确认有没有明显风险</span>
        </div>
        <div className="flex gap-2">
          <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" style={{ color: "#D4A853" }} />
          <span>最后让它帮你验证一遍</span>
        </div>
      </div>

      <div className="space-y-1.5 border-t px-3 py-3" style={{ borderColor: "#171717" }}>
        <div className="text-[10px] uppercase tracking-wider" style={{ color: "#666" }}>
          可复制的验收提示词
        </div>
        {ACCEPTANCE_PROMPTS.map((prompt, index) => (
          <button
            key={prompt}
            onClick={() => copyPrompt(prompt, index)}
            className="flex w-full items-start gap-2 rounded-md border px-3 py-2 text-left text-xs transition-colors hover:text-foreground"
            style={{ background: "#0f0f0f", borderColor: "#1a1a1a", color: "#aaa" }}
          >
            <Copy className="mt-0.5 size-3.5 shrink-0" style={{ color: copiedIndex === index ? "#4A9E6B" : "#777" }} />
            <span>{copiedIndex === index ? "已复制：" : ""}{prompt}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
