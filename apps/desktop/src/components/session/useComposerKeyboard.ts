import { useCallback, type MutableRefObject } from "react";
import { COMPOSER_COMMANDS } from "./composerCommands";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";

interface UseComposerKeyboardOptions {
  activeSuggestionIndex: number;
  addChip: (type: ComposerChip["type"], value: string) => void;
  atResults: string[];
  chipsCount: number;
  closeModelMenu: () => void;
  closeSuggestions: () => void;
  composingRef: MutableRefObject<boolean>;
  onSend: () => void;
  removeLastChip: () => void;
  setActiveSuggestionIndex: (updater: (index: number) => number) => void;
  showSuggestions: ComposerMenuMode;
  valueRef: MutableRefObject<string>;
}

export function useComposerKeyboard({
  activeSuggestionIndex,
  addChip,
  atResults,
  chipsCount,
  closeModelMenu,
  closeSuggestions,
  composingRef,
  onSend,
  removeLastChip,
  setActiveSuggestionIndex,
  showSuggestions,
  valueRef,
}: UseComposerKeyboardOptions) {
  const commitActiveSuggestion = useCallback(() => {
    if (showSuggestions === "/") {
      const command = COMPOSER_COMMANDS[activeSuggestionIndex];
      if (!command) return false;
      addChip("command", command.text);
      return true;
    }

    if (showSuggestions === "@") {
      const file = atResults[activeSuggestionIndex];
      if (!file) return false;
      addChip("file", file);
      return true;
    }

    return false;
  }, [activeSuggestionIndex, addChip, atResults, showSuggestions]);

  return useCallback((event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (composingRef.current || event.nativeEvent.isComposing) return;

    const suggestionCount = showSuggestions === "/"
      ? COMPOSER_COMMANDS.length
      : showSuggestions === "@"
        ? atResults.length
        : 0;

    if (showSuggestions && suggestionCount > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActiveSuggestionIndex((index) => (index + 1) % suggestionCount);
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setActiveSuggestionIndex((index) => (index - 1 + suggestionCount) % suggestionCount);
        return;
      }

      if ((event.key === "Enter" && !event.shiftKey) || event.key === "Tab") {
        event.preventDefault();
        if (commitActiveSuggestion()) return;
      }
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSend();
    }

    if (event.key === "Escape") {
      closeSuggestions();
      closeModelMenu();
    }

    const currentValue = valueRef.current;
    if ((event.key === "Backspace" || event.key === "Delete") && currentValue === "" && chipsCount > 0) {
      event.preventDefault();
      removeLastChip();
    }
  }, [
    atResults.length,
    chipsCount,
    closeModelMenu,
    closeSuggestions,
    commitActiveSuggestion,
    composingRef,
    onSend,
    removeLastChip,
    setActiveSuggestionIndex,
    showSuggestions,
    valueRef,
  ]);
}
