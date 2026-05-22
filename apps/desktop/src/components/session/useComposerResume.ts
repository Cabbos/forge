import { useCallback, useState } from "react";

interface UseComposerResumeOptions {
  isRunning: boolean;
  onFocusTextarea: () => void;
  resume: (sessionId: string) => Promise<string>;
  sessionId: string;
}

export function useComposerResume({
  isRunning,
  onFocusTextarea,
  resume,
  sessionId,
}: UseComposerResumeOptions) {
  const [isResuming, setIsResuming] = useState(false);
  const [resumeError, setResumeError] = useState("");

  const handleResume = useCallback(async () => {
    if (isRunning || isResuming) return;
    setResumeError("");
    setIsResuming(true);
    try {
      await resume(sessionId);
      setTimeout(onFocusTextarea, 0);
    } catch (error) {
      setResumeError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsResuming(false);
    }
  }, [isRunning, isResuming, onFocusTextarea, resume, sessionId]);

  return {
    handleResume,
    isResuming,
    resumeError,
  };
}
