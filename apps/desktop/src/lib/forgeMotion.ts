import gsap from "gsap";
import { useGSAP } from "@gsap/react";

gsap.registerPlugin(useGSAP);

export { gsap, useGSAP };

export const forgeMotion = {
  message: {
    duration: 0.28,
    ease: "power3.out",
  },
  evidence: {
    duration: 0.2,
    ease: "power2.out",
  },
  surface: {
    duration: 0.24,
    ease: "power3.out",
  },
} as const;

export function prefersReducedMotion() {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}
