import { useCallback, useEffect, useRef, useState } from "react";
import { searchWorkspaceFiles } from "@/lib/tauri";
import type { ComposerMenuMode } from "./composerTypes";

interface UseComposerSuggestionsOptions {
  sessionId: string;
  workingDir?: string | null;
  onCloseModelMenu: () => void;
  onFocusTextarea: () => void;
}

export function useComposerSuggestions({
  sessionId,
  workingDir,
  onCloseModelMenu,
  onFocusTextarea,
}: UseComposerSuggestionsOptions) {
  const [showSuggestions, setShowSuggestions] = useState<ComposerMenuMode>(null);
  const [activeSuggestionIndex, setActiveSuggestionIndex] = useState(0);
  const [atResults, setAtResults] = useState<string[]>([]);
  const atSearchRequestRef = useRef(0);

  useEffect(() => {
    setActiveSuggestionIndex(0);
  }, [showSuggestions, atResults.length]);

  useEffect(() => {
    return () => {
      atSearchRequestRef.current += 1;
    };
  }, []);

  const closeSuggestions = useCallback(() => {
    atSearchRequestRef.current += 1;
    setShowSuggestions(null);
    setAtResults([]);
  }, []);

  const syncSuggestionsForInput = useCallback((inputValue: string, cursorPosition: number) => {
    const beforeCursor = inputValue.slice(0, cursorPosition);
    const lastWord = beforeCursor.split(/\s/).pop() || "";

    if (lastWord.startsWith("@") && lastWord.length >= 1) {
      onCloseModelMenu();
      setShowSuggestions("@");
      const requestId = atSearchRequestRef.current + 1;
      atSearchRequestRef.current = requestId;
      searchWorkspaceFiles(lastWord.slice(1), sessionId, workingDir)
        .then((results) => {
          if (atSearchRequestRef.current === requestId) setAtResults(results);
        })
        .catch(() => {
          if (atSearchRequestRef.current === requestId) setAtResults([]);
        });
      return;
    }

    if (lastWord === "/") {
      onCloseModelMenu();
      atSearchRequestRef.current += 1;
      setAtResults([]);
      setShowSuggestions("/");
      return;
    }

    closeSuggestions();
  }, [closeSuggestions, onCloseModelMenu, sessionId, workingDir]);

  const toggleSuggestion = useCallback((mode: Exclude<ComposerMenuMode, null>) => {
    onFocusTextarea();
    onCloseModelMenu();
    setShowSuggestions((current) => {
      const next = current === mode ? null : mode;
      if (next !== "@") {
        atSearchRequestRef.current += 1;
        setAtResults([]);
      }
      return next;
    });
  }, [onCloseModelMenu, onFocusTextarea]);

  return {
    activeSuggestionIndex,
    atResults,
    closeSuggestions,
    setActiveSuggestionIndex,
    showSuggestions,
    syncSuggestionsForInput,
    toggleSuggestion,
  };
}
