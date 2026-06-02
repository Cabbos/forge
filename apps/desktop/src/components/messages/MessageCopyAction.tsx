import { useEffect, useRef, useState } from "react";
import type { MouseEvent } from "react";
import { Check, Copy } from "lucide-react";
import { ForgeIconButton } from "@/components/primitives/icon-button";

export function MessageCopyAction({ text, label }: { text: string; label: "回复" | "提问" }) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const actionLabel = copied ? `已复制${label}` : `复制${label}`;

  useEffect(() => () => {
    if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
  }, []);

  const copyMessage = async (event: MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    await navigator.clipboard?.writeText(text);
    setCopied(true);
    if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
    resetTimerRef.current = setTimeout(() => setCopied(false), 1200);
  };

  return (
    <ForgeIconButton
      data-testid="message-copy-action"
      className="forge-message-copy-action"
      aria-label={actionLabel}
      title={actionLabel}
      onClick={copyMessage}
    >
      {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
    </ForgeIconButton>
  );
}
