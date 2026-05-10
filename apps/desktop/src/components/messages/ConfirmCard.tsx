import { useState } from "react";
import { ShieldAlert, Check, X } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";

export function ConfirmCard({ block }: { block: BlockState }) {
  const [responded, setResponded] = useState(false);
  const [answer, setAnswer] = useState<boolean | null>(null);
  const question = block.content || "Allow this operation?";
  const kind = (block.metadata.kind as string) || "operation";

  const handleResponse = async (approved: boolean) => {
    setResponded(true);
    setAnswer(approved);
    try {
      await confirmResponse(block.block_id, approved);
    } catch (e) {
      console.error("confirmResponse failed:", e);
    }
  };

  return (
    <div className="my-4 flex justify-start">
      <div className="max-w-[85%] min-w-[320px] rounded-xl border border-warning/30 bg-warning/5 overflow-hidden">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-b border-warning/20 bg-warning/10">
          <ShieldAlert className="size-4 text-foreground" />
          <span className="text-xs font-semibold text-foreground uppercase tracking-wide">
            {kind.replace(/_/g, " ")}
          </span>
        </div>

        {/* Question */}
        <div className="px-4 py-3">
          <p className="text-sm text-foreground leading-relaxed">{question}</p>
        </div>

        {/* Actions */}
        {responded ? (
          <div className="px-4 py-2.5 border-t border-border flex items-center gap-2">
            <span className={`text-xs font-medium ${answer ? "text-green-500" : "text-destructive"}`}>
              {answer ? "Approved" : "Denied"}
            </span>
          </div>
        ) : (
          <div className="px-4 py-2.5 border-t border-border flex items-center gap-2">
            <button
              onClick={() => handleResponse(true)}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium bg-primary text-primary-foreground hover:brightness-110 transition-all"
            >
              <Check className="size-3.5" />
              Allow
            </button>
            <button
              onClick={() => handleResponse(false)}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium bg-destructive text-destructive-foreground hover:brightness-110 transition-all"
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
