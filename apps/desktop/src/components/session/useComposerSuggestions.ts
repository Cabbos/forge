import { useCallback, useEffect, useState } from "react";
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

  useEffect(() => {
    setActiveSuggestionIndex(0);
  }, [showSuggestions, atResults.length]);

  const closeSuggestions = useCallback(() => {
    setShowSuggestions(null);
  }, []);

  const syncSuggestionsForInput = useCallback((inputValue: string, cursorPosition: number) => {
    const beforeCursor = inputValue.slice(0, cursorPosition);
    const lastWord = beforeCursor.split(/\s/).pop() || "";

    if (lastWord.startsWith("@") && lastWord.length >= 1) {
      onCloseModelMenu();
      setShowSuggestions("@");
      searchWorkspaceFiles(lastWord.slice(1), sessionId, workingDir)
        .then(setAtResults)
        .catch(() => setAtResults([]));
      return;
    }

    if (lastWord === "/") {
      onCloseModelMenu();
      setShowSuggestions("/");
      return;
    }

    closeSuggestions();
  }, [closeSuggestions, onCloseModelMenu, sessionId, workingDir]);

  const toggleSuggestion = useCallback((mode: Exclude<ComposerMenuMode, null>) => {
    onFocusTextarea();
    onCloseModelMenu();
    setShowSuggestions((current) => current === mode ? null : mode);
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
