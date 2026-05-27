import { ClipboardCheck } from "lucide-react";
import { useRef, useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { useStore } from "@/store";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { deriveDeliverySummaryPresentation } from "@/components/messages/deliverySummaryPresentation";
import { DeliveryPrimaryAction, DeliverySummaryItemView } from "@/components/messages/DeliverySummaryViews";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function DeliverySummaryCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [loadedPrompt, setLoadedPrompt] = useState<string | null>(null);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const { view, projectName, panelTone, iconTone } = deriveDeliverySummaryPresentation(block.metadata.summary);

  const loadPrompt = (prompt: string) => {
    setPendingInput(prompt);
    setLoadedPrompt(prompt);
    window.setTimeout(() => setLoadedPrompt(null), 1200);
  };
  const runPrimaryAction = () => {
    if (view.primaryAction.action === "open_records") {
      window.dispatchEvent(new CustomEvent("open-hub", { detail: { section: "records" } }));
      return;
    }
    if (view.primaryAction.prompt) loadPrompt(view.primaryAction.prompt);
  };
  const loaded = loadedPrompt === view.primaryAction.prompt;

  useGSAP(() => {
    if (prefersReducedMotion()) return;
    const items = gsap.utils.toArray<HTMLElement>(".forge-delivery-item, .forge-delivery-action", rootRef.current ?? undefined);
    if (items.length === 0) return;

    gsap.fromTo(
      items,
      { autoAlpha: 0, y: 5 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        stagger: 0.025,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: rootRef, dependencies: [view.items.length, view.primaryAction.label] });

  return (
    <div ref={rootRef}>
      <MessagePanel
        tone={panelTone}
        className="forge-delivery-card"
        data-forge-motion="delivery-card"
        data-delivery-tone={view.tone}
      >
        <MessagePanelHeader
          icon={<ForgeIcon icon={ClipboardCheck} tone={iconTone} contained={false} className="size-3.5" />}
          title="本轮交付"
          meta={projectName ? <span>{projectName}</span> : null}
        />

        <div data-testid="delivery-summary-grid" className="forge-delivery-grid">
          {view.items.map((item) => (
            <DeliverySummaryItemView key={`${item.kind}-${item.label}`} item={item} />
          ))}
        </div>

        <DeliveryPrimaryAction
          action={view.primaryAction}
          loaded={loaded}
          sessionId={sessionId}
          onClick={runPrimaryAction}
        />
      </MessagePanel>
    </div>
  );
}
