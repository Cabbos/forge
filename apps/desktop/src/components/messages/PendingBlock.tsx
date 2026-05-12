import { useEffect, useState } from "react";
import forgeMark from "@/assets/forge-mark.svg";

const HINTS = [
  "正在理解你的需求...",
  "正在查看相关文件...",
  "正在判断哪里需要改...",
  "正在整理下一步操作...",
  "正在检查可能的风险...",
  "正在把技术细节翻译成人话...",
  "正在确认改动是否合理...",
  "快好了，正在收尾...",
];

export function PendingBlock() {
  const [hint, setHint] = useState(HINTS[0]);
  const [fadeOut, setFadeOut] = useState(false);

  useEffect(() => {
    const rotate = setInterval(() => {
      setFadeOut(true);
      setTimeout(() => { setHint(HINTS[Math.floor(Math.random() * HINTS.length)]); setFadeOut(false); }, 500);
    }, 3500);
    return () => clearInterval(rotate);
  }, []);

  return (
    <div className="flex flex-col items-center gap-3 py-8 select-none">
      <div className="relative flex size-10 items-center justify-center">
        <div className="absolute inset-0 rounded-lg border border-primary/20 bg-primary/5 animate-pulse" />
        <img src={forgeMark} alt="" className="relative size-8 rounded-md" />
      </div>
      <span className="text-[12px] transition-opacity duration-500 text-center"
        style={{ color: fadeOut ? "transparent" : "var(--muted-foreground)" }}>
        {hint}
      </span>
    </div>
  );
}
