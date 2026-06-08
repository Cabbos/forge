import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, X } from "lucide-react";
import { confirmResolvedLabel } from "@/components/messages/confirmPresentation";

export function ConfirmActionBar({
  responded,
  answer,
  onResponse,
}: {
  responded: boolean;
  answer: boolean | null;
  onResponse: (approved: boolean) => void;
}) {
  if (responded) {
    return (
      <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
        <span className="forge-confirm-resolved" data-state={answer ? "approved" : "cancelled"}>
          {confirmResolvedLabel(answer)}
        </span>
      </div>
    );
  }

  return (
    <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
      <ButtonPrimitive
        type="button"
        data-testid="confirm-approve"
        onClick={(event) => {
          event.stopPropagation();
          onResponse(true);
        }}
        className="forge-confirm-button"
        data-variant="approve"
      >
        <Check className="size-3.5" />
        继续
      </ButtonPrimitive>
      <ButtonPrimitive
        type="button"
        data-testid="confirm-cancel"
        onClick={(event) => {
          event.stopPropagation();
          onResponse(false);
        }}
        className="forge-confirm-button"
        data-variant="cancel"
      >
        <X className="size-3.5" />
        取消
      </ButtonPrimitive>
    </div>
  );
}
