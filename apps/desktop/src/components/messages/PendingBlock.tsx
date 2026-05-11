import { useEffect, useState } from "react";
import { WhaleSVG } from "./WhaleSVG";

const HINTS = [
  "Thinking...",
  "好的代码不需要注释",
  "Reading the codebase...",
  "少即是多 · Less is more",
  "Analyzing context...",
  "每一个 tool 调用都是可审查的",
  "Planning the approach...",
  "Crafting the solution...",
  "Almost there...",
  "YAGNI",
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
