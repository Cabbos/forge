import { useEffect, useState } from "react";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { ShellCardDetail } from "@/components/messages/ShellCardDetail";
import { ShellCardHeader } from "@/components/messages/ShellCardHeader";
import { deriveShellView } from "./processShellPresentation";

export function ShellCard({ block }: { block: BlockState }) {
  const shellView = deriveShellView(block);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    if (block.isComplete && shellView.isError) setExpanded(true);
  }, [block.isComplete, shellView.isError]);

  return (
    <div>
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <ShellCardHeader
          command={shellView.command}
          expanded={expanded}
          exitCode={shellView.exitCode}
          isError={shellView.isError}
          isRunning={shellView.isRunning}
          state={shellView.state}
          tone={shellView.tone}
        />
        <CollapsibleContent>
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
