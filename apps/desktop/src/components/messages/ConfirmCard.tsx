import { useEffect, useState } from "react";
import type { BlockState, PermissionLedgerEvent } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";
import { parseWriteBoundary } from "@/lib/write-boundary";
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

export function ConfirmCard({ block }: { block: BlockState; sessionId?: string }) {
  const interrupted = block.metadata.confirm_interrupted === true;
  const alreadyResolved = block.metadata.confirmed === true;
  const savedAnswer = block.metadata.answer as boolean | undefined;
  const [responded, setResponded] = useState(alreadyResolved);
  const [answer, setAnswer] = useState<boolean | null>(savedAnswer ?? null);
  const boundary = parseWriteBoundary(block.metadata.boundary);
  const permissionEvidence = parsePermissionEvidence(block.metadata.permission_evidence);
  const promptView = deriveConfirmPromptView(block.content, block.metadata.kind);

  useEffect(() => {
    setResponded(block.metadata.confirmed === true);
    setAnswer(typeof block.metadata.answer === "boolean" ? block.metadata.answer : null);
  }, [block.block_id, block.metadata.answer, block.metadata.confirmed]);

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
  };

  if (boundary) {
    if (interrupted) {
      return <ConfirmBoundaryInterruptedView boundary={boundary} evidence={permissionEvidence} />;
    }

    if (responded) {
      return <ConfirmBoundaryResolvedView boundary={boundary} answer={answer} evidence={permissionEvidence} />;
    }

    return <ConfirmBoundaryPendingView boundary={boundary} evidence={permissionEvidence} onResponse={handleResponse} />;
  }

  if (interrupted) {
    return <ConfirmPromptInterruptedView prompt={promptView} evidence={permissionEvidence} />;
  }

  return (
    <ConfirmPromptView
      prompt={promptView}
      responded={responded}
      answer={answer}
      evidence={permissionEvidence}
      onResponse={handleResponse}
    />
  );
}

function parsePermissionEvidence(value: unknown): PermissionLedgerEvent | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const record = value as Partial<PermissionLedgerEvent>;
  if (
    typeof record.kind !== "string" ||
    typeof record.workspace_path !== "string" ||
    typeof record.risk_tier !== "string" ||
    !Array.isArray(record.affected_files) ||
    typeof record.operation !== "string" ||
    typeof record.permission_mode !== "string" ||
    typeof record.reason !== "string"
  ) {
    return null;
  }
  return record as PermissionLedgerEvent;
}
