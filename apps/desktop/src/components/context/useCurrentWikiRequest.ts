import { useCallback, useRef } from "react";

export function useCurrentWikiRequest(currentProjectPath: string, sessionId: string | null) {
  const currentProjectPathRef = useRef(currentProjectPath);
  const sessionIdRef = useRef(sessionId);

  currentProjectPathRef.current = currentProjectPath;
  sessionIdRef.current = sessionId;

  return useCallback((projectAtStart: string, sessionAtStart: string | null) => {
    return currentProjectPathRef.current === projectAtStart && sessionIdRef.current === sessionAtStart;
  }, []);
}
