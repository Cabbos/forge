import type { RefObject } from "react";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function useMessageEntryMotion(laneRef: RefObject<HTMLDivElement | null>, blockCount: number) {
  useGSAP(() => {
    if (prefersReducedMotion()) return;

    const lane = laneRef.current;
    if (!lane) return;

    const messageBlocks = gsap.utils.toArray<HTMLElement>("[data-testid='message-block']", lane);
    const latest = messageBlocks[messageBlocks.length - 1];
    if (!latest || latest.dataset.forgeMotionSeen === "true") return;

    latest.dataset.forgeMotionSeen = "true";
    gsap.fromTo(
      latest,
      { autoAlpha: 0, y: 8, scale: 0.996 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.message.duration,
        ease: forgeMotion.message.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: laneRef, dependencies: [blockCount] });
}
