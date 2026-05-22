import { useState } from "react";
import { Check, Copy } from "lucide-react";

interface ReaderCaptionActionProps {
  text: string;
  idleLabel: string;
  copiedLabel?: string;
}

export function ReaderCaptionAction({
  text,
  idleLabel,
  copiedLabel = "已复制",
}: ReaderCaptionActionProps) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      setCopied(false);
    }
  };

  return (
    <button
      type="button"
      onClick={copy}
      className="forge-caption-action"
      aria-label={copied ? copiedLabel : idleLabel}
      title={copied ? copiedLabel : idleLabel}
    >
      {copied ? <Check className="size-3.5" style={{ color: "var(--forge-icon-safety)" }} /> : <Copy className="size-3.5" />}
    </button>
  );
}
