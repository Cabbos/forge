import { cn } from "@/lib/utils";

interface Props {
  done?: boolean;
  size?: number;
  animate?: boolean;
}

/** Clean whale icon. Animated swim when streaming, static green when done. */
export function WhaleSVG({ done = false, size = 16, animate = false }: Props) {
  const h = size * 0.5;
  const color = done ? "#4A9E6B" : "#5B9BD5";
  const opacity = done ? 0.35 : 0.7;

  return (
    <svg width={size} height={h} viewBox="0 0 36 18" fill="none"
      className={cn(animate && !done && "animate-[swim_2s_ease-in-out_infinite]")}
      style={{ flexShrink: 0 }}>
      {/* Body: smooth whale silhouette */}
      <path d="M28 9 Q28 11 26 12.5 Q22 15 16 15 Q10 15 6 13 Q2 11 1.5 9 Q1 7 2.5 5.5 Q5 3 10 2.5 Q16 2 22 4 Q26 5.5 28 7.5 Q29 8.5 28 9Z"
        fill={color} opacity={opacity} />
      {/* Eye */}
      <circle cx="9" cy="8" r="1.2" fill="#0D0D0D" />
      {/* Dorsal fin */}
      <path d="M19 4 Q20 1 22 3.5" fill={color} opacity={opacity * 0.8} />
      {/* Tail */}
      <path d="M28 8 Q31 5 33 4 Q34 8 33 11 Q31 10 28 9Z" fill={color} opacity={opacity * 0.9} />
      {/* Water spout — only when animating */}
      {animate && !done && (
        <>
          <circle cx="5" cy="0" r="0.8" fill="#5B9BD5" opacity="0.3">
            <animate attributeName="cy" values="2;-2;2" dur="0.8s" repeatCount="indefinite" />
            <animate attributeName="opacity" values="0.3;0.1;0.3" dur="0.8s" repeatCount="indefinite" />
          </circle>
          <circle cx="6.5" cy="0.5" r="0.6" fill="#5B9BD5" opacity="0.25">
            <animate attributeName="cy" values="2;-3;2" dur="0.8s" begin="0.2s" repeatCount="indefinite" />
            <animate attributeName="opacity" values="0.25;0.05;0.25" dur="0.8s" begin="0.2s" repeatCount="indefinite" />
          </circle>
          <circle cx="3.5" cy="0.8" r="0.5" fill="#5B9BD5" opacity="0.2">
            <animate attributeName="cy" values="2;-1.5;2" dur="0.8s" begin="0.4s" repeatCount="indefinite" />
            <animate attributeName="opacity" values="0.2;0.05;0.2" dur="0.8s" begin="0.4s" repeatCount="indefinite" />
          </circle>
        </>
      )}
    </svg>
  );
}
