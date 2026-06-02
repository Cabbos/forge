import { useRef } from "react";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function useSettingsDialogMotion(dialogOpen: boolean) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useGSAP(() => {
    if (!dialogOpen || prefersReducedMotion()) return;
    const dialog = dialogRef.current;
    if (!dialog) return;

    const rows = gsap.utils.toArray<HTMLElement>(
      "[data-forge-motion='settings-entry']",
      dialog,
    );
    const timeline = gsap.timeline();
    timeline.fromTo(
      dialog,
      { autoAlpha: 0, y: 10, scale: 0.985 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
    if (rows.length > 0) {
      timeline.fromTo(
        rows,
        { autoAlpha: 0, y: 5 },
        {
          autoAlpha: 1,
          y: 0,
          duration: forgeMotion.evidence.duration,
          ease: forgeMotion.evidence.ease,
          stagger: 0.025,
          clearProps: "transform,opacity,visibility",
        },
        "-=0.1",
      );
    }
  }, { dependencies: [dialogOpen] });

  return dialogRef;
}
