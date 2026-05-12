import { useEffect, useState } from "react";
import { WhaleSVG } from "./WhaleSVG";

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
      <div className="relative overflow-hidden" style={{ width: 100, height: 48 }}>
        <div className="absolute left-0 right-0" style={{ bottom: 8, borderTop: "1px solid rgba(91,155,213,0.12)" }} />
        <div className="absolute animate-[dolphin-jump_2.2s_ease-in-out_infinite]" style={{ left: -24, bottom: 6 }}>
          <WhaleSVG animate size={24} />
        </div>
      </div>
      <span className="text-[12px] transition-opacity duration-500 text-center"
        style={{ color: fadeOut ? "transparent" : "#555" }}>
        {hint}
      </span>
    </div>
  );
}
