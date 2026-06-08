import { useEffect, useRef } from "react";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";
import { HubPanelShell } from "./HubPanelShell";
import { useHubPanelData, type HubPanelSection } from "./useHubPanelData";

interface HubPanelProps {
  open: boolean;
  initialSection?: HubPanelSection | null;
  onOpenChange: (open: boolean) => void;
}

export function HubPanel({ open, initialSection, onOpenChange }: HubPanelProps) {
  const panelRef = useRef<HTMLElement>(null);
  const panelData = useHubPanelData({ initialSection, open });

  useGSAP(() => {
    if (!open || prefersReducedMotion()) return;
    const panel = panelRef.current;
    if (!panel) return;

    const sections = gsap.utils.toArray<HTMLElement>("[data-forge-motion='archive-section']", panel);
    const timeline = gsap.timeline();
    timeline.fromTo(
      panel,
      { autoAlpha: 0, x: 18 },
      {
        autoAlpha: 1,
        x: 0,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
    if (sections.length > 0) {
      timeline.fromTo(
        sections,
        { autoAlpha: 0, y: 5 },
        {
          autoAlpha: 1,
          y: 0,
          duration: forgeMotion.evidence.duration,
          ease: forgeMotion.evidence.ease,
          stagger: 0.025,
          clearProps: "transform,opacity,visibility",
        },
        "-=0.08",
      );
    }
  }, { scope: panelRef, dependencies: [open] });

  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onOpenChange(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onOpenChange, open]);

  if (!open) return null;

  return (
    <HubPanelShell
      {...panelData}
      panelRef={panelRef}
      onClose={() => onOpenChange(false)}
    />
  );
}
