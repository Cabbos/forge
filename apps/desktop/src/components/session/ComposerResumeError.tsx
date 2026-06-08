import { AlertCircle } from "lucide-react";

interface ComposerResumeErrorProps {
  message: string;
}

export function ComposerResumeError({ message }: ComposerResumeErrorProps) {
  if (!message) return null;

  return (
    <div data-testid="composer-error" role="status" aria-live="polite" className="forge-composer-error">
      <AlertCircle className="size-3.5 shrink-0" />
      <span className="min-w-0 truncate">{message}</span>
    </div>
  );
}
