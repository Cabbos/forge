import { useEffect, useRef, useState } from "react";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { ShellCardDetail } from "@/components/messages/ShellCardDetail";
import { ShellCardHeader } from "@/components/messages/ShellCardHeader";
import { deriveShellView } from "./processShellPresentation";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function ShellCard({ block }: { block: BlockState }) {
  const shellView = deriveShellView(block);
  const [expanded, setExpanded] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (block.isComplete && shellView.isError) setExpanded(true);
  }, [block.isComplete, shellView.isError]);

  useGSAP(() => {
    if (!expanded || prefersReducedMotion()) return;

    const detail = rootRef.current?.querySelector<HTMLElement>("[data-forge-motion='shell-detail']");
    if (!detail) return;

    gsap.fromTo(
      detail,
      { autoAlpha: 0, y: -4 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: rootRef, dependencies: [expanded] });

  return (
    <div ref={rootRef} className="shell-reel">
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <div className="shell-reel-header">
          <div className="shell-reel-body">
            <ShellCardHeader
              command={shellView.command}
              expanded={expanded}
              exitCode={shellView.exitCode}
              isError={shellView.isError}
              isRunning={shellView.isRunning}
              state={shellView.state}
              tone={shellView.tone}
            />
          </div>
        </div>
        <CollapsibleContent data-forge-motion="shell-detail">
          <ShellCardDetail
            command={shellView.command}
            output={shellView.output}
            outputSections={shellView.outputSections}
            tone={shellView.tone}
          />
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
