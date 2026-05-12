import { useState } from "react";
import { ShieldAlert, Check, X } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";
import { useStore } from "@/store";

export function ConfirmCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const updateBlock = useStore((s) => s.updateBlock);
  const alreadyResolved = block.metadata.confirmed === true;
  const savedAnswer = block.metadata.answer as boolean | undefined;
  const [responded, setResponded] = useState(alreadyResolved);
  const [answer, setAnswer] = useState<boolean | null>(savedAnswer ?? null);
  const question = block.content || "Allow this operation?";
  const kind = (block.metadata.kind as string) || "operation";

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

  return (
    <div className="mb-3">
      <div className="max-w-full rounded-xl border overflow-hidden" style={{ borderColor: "rgba(212,168,83,0.2)", background: "rgba(212,168,83,0.04)" }}>
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-b" style={{ borderColor: "rgba(212,168,83,0.15)", background: "rgba(212,168,83,0.06)" }}>
          <ShieldAlert className="size-4" style={{ color: "#D4A853" }} />
          <span className="text-xs font-semibold uppercase tracking-wide" style={{ color: "#D4A853" }}>
            {kind.replace(/_/g, " ")}
          </span>
        </div>

        {/* Question */}
        <div className="px-4 py-3">
          <p className="text-sm leading-relaxed" style={{ color: "#ccc" }}>{question}</p>
        </div>

        {/* Actions */}
        {responded ? (
          <div className="px-4 py-2.5 border-t border-border flex items-center gap-2">
            <span className={`text-xs font-medium ${answer ? "text-green-500" : "text-destructive"}`}>
              {answer ? "Approved" : "Denied"}
            </span>
          </div>
        ) : (
          <div className="px-4 py-2.5 border-t flex items-center gap-2" style={{ borderColor: "rgba(212,168,83,0.1)" }}>
            <button
              onClick={(e) => { e.stopPropagation(); handleResponse(true); }}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium transition-all cursor-pointer"
              style={{ background: "#D4A853", color: "#0D0D0D" }}
            >
              <Check className="size-3.5" />
              Allow
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); handleResponse(false); }}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium transition-all cursor-pointer"
              style={{ background: "#D47777", color: "#0D0D0D" }}
            >
              <X className="size-3.5" />
              Deny
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
