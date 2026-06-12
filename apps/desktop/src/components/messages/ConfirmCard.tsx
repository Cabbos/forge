import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";
import { parseWriteBoundary } from "@/lib/write-boundary";
import { useStore } from "@/store";
import {
  ConfirmBoundaryInterruptedView,
  ConfirmBoundaryPendingView,
  ConfirmBoundaryResolvedView,
  ConfirmPromptInterruptedView,
  ConfirmPromptView,
} from "@/components/messages/ConfirmViews";
import {
  deriveConfirmPromptView,
} from "@/components/messages/confirmPresentation";

export function ConfirmCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const updateBlock = useStore((s) => s.updateBlock);
  const interrupted = block.metadata.confirm_interrupted === true;
  const alreadyResolved = block.metadata.confirmed === true;
  const savedAnswer = block.metadata.answer as boolean | undefined;
  const [responded, setResponded] = useState(alreadyResolved);
  const [answer, setAnswer] = useState<boolean | null>(savedAnswer ?? null);
  const boundary = parseWriteBoundary(block.metadata.boundary);
  const promptView = deriveConfirmPromptView(block.content, block.metadata.kind);

  const handleResponse = async (approved: boolean) => {
    setResponded(true);
    setAnswer(approved);
    try {
      await confirmResponse(block.block_id, approved);
    } catch (e) {
      console.error("confirmResponse failed:", e);
      // Revert UI on error
      setResponded(false);
      setAnswer(null);
      return;
    }
    // Persist confirmation state so it survives session reload
    if (sessionId) {
      updateBlock(sessionId, block.block_id, {
        metadata: { ...block.metadata, confirmed: true, answer: approved },
      });
    }
  };

  if (boundary) {
    if (interrupted) {
      return <ConfirmBoundaryInterruptedView boundary={boundary} />;
    }

    if (responded) {
      return <ConfirmBoundaryResolvedView boundary={boundary} answer={answer} />;
    }

    return <ConfirmBoundaryPendingView boundary={boundary} onResponse={handleResponse} />;
  }

  if (interrupted) {
    return <ConfirmPromptInterruptedView prompt={promptView} />;
  }

  return (
    <ConfirmPromptView
      prompt={promptView}
      responded={responded}
      answer={answer}
      onResponse={handleResponse}
    />
  );
}
