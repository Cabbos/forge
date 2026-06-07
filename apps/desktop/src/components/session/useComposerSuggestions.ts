import { useCallback, useEffect, useState } from "react";
import { useSearchWorkspaceFilesQuery } from "@/hooks/queries/useSearchWorkspaceFilesQuery";
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
  const [searchTerm, setSearchTerm] = useState("");

  const {
    data: searchResults = [],
    isFetching: searchLoading,
  } = useSearchWorkspaceFilesQuery(
    searchTerm,
    sessionId,
    workingDir,
    showSuggestions === "@",
  );

  const atResults = showSuggestions === "@" ? searchResults : [];

  useEffect(() => {
    setActiveSuggestionIndex(0);
  }, [showSuggestions, atResults.length]);

  const closeSuggestions = useCallback(() => {
    setShowSuggestions(null);
    setSearchTerm("");
  }, []);

  const syncSuggestionsForInput = useCallback((inputValue: string, cursorPosition: number) => {
    const beforeCursor = inputValue.slice(0, cursorPosition);
    const lastWord = beforeCursor.split(/\s/).pop() || "";

    if (lastWord.startsWith("@") && lastWord.length >= 1) {
      onCloseModelMenu();
      setShowSuggestions("@");
      setSearchTerm(lastWord.slice(1));
      return;
    }

    if (lastWord === "/") {
      onCloseModelMenu();
      setSearchTerm("");
      setShowSuggestions("/");
      return;
    }

    closeSuggestions();
  }, [closeSuggestions, onCloseModelMenu]);

  const toggleSuggestion = useCallback((mode: Exclude<ComposerMenuMode, null>) => {
    onFocusTextarea();
    onCloseModelMenu();
    setShowSuggestions((current) => {
      const next = current === mode ? null : mode;
      if (next !== "@") {
        setSearchTerm("");
      }
      return next;
    });
  }, [onCloseModelMenu, onFocusTextarea]);

  return {
    activeSuggestionIndex,
    atResults,
    closeSuggestions,
    searchLoading,
    setActiveSuggestionIndex,
    showSuggestions,
    syncSuggestionsForInput,
    toggleSuggestion,
  };
}
