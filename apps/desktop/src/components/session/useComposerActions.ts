import { useCallback, useState, type MutableRefObject } from "react";
import { useSession } from "@/hooks/useSession";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";
import { useComposerInputHandlers } from "./useComposerInputHandlers";
import { useComposerKeyboard } from "./useComposerKeyboard";
import { useComposerResume } from "./useComposerResume";
import { useComposerSubmit } from "./useComposerSubmit";

type ComposerDraftSetter = (nextValue: string | ((current: string) => string)) => void;

interface UseComposerActionsOptions {
  activeSuggestionIndex: number;
  addChip: (type: ComposerChip["type"], value: string) => void;
  adjustHeight: () => void;
  atResults: string[];
  chips: ComposerChip[];
  clearChips: () => void;
  closeModelMenu: () => void;
  closeSuggestions: () => void;
  composingRef: MutableRefObject<boolean>;
  focusTextarea: () => void;
  isRunning: boolean;
  removeLastChip: () => void;
  resetDraft: () => void;
  sessionId: string;
  setActiveSuggestionIndex: (updater: (index: number) => number) => void;
  setValue: ComposerDraftSetter;
  showSuggestions: ComposerMenuMode;
  syncSuggestionsForInput: (inputValue: string, cursorPosition: number) => void;
  toggleModelMenu: () => void;
  value: string;
  valueRef: MutableRefObject<string>;
  workingDir?: string | null;
}

export function useComposerActions({
  activeSuggestionIndex,
  addChip,
  adjustHeight,
  atResults,
  chips,
  clearChips,
  closeModelMenu,
  closeSuggestions,
  composingRef,
  focusTextarea,
  isRunning,
  removeLastChip,
  resetDraft,
  sessionId,
  setActiveSuggestionIndex,
  setValue,
  showSuggestions,
  syncSuggestionsForInput,
  toggleModelMenu,
  value,
  valueRef,
  workingDir,
}: UseComposerActionsOptions) {
  const { send, stop, resume, compact } = useSession();
  const [isCompacting, setIsCompacting] = useState(false);
  const runCompactWithFeedback = useCallback(async () => {
    if (!isRunning || isCompacting) return;
    setIsCompacting(true);
    try {
      await compact(sessionId);
    } finally {
      setIsCompacting(false);
    }
  }, [compact, isCompacting, isRunning, sessionId]);
  const {
    handleResume,
    isResuming,
    resumeError,
  } = useComposerResume({
    isRunning,
    onFocusTextarea: focusTextarea,
    resume,
    sessionId,
  });
  const {
    handleChange,
    handleCompositionEnd,
    handleCompositionStart,
    handleStop,
    handleToggleModelMenu,
  } = useComposerInputHandlers({
    adjustHeight,
    closeSuggestions,
    composingRef,
    sessionId,
    setValue,
    stop,
    syncSuggestionsForInput,
    toggleModelMenu,
    valueRef,
  });
  const handleSend = useComposerSubmit({
    chips,
    isRunning,
    onClearChips: clearChips,
    onResetDraft: resetDraft,
    compact: runCompactWithFeedback,
    send,
    sessionId,
    value,
    workingDir,
  });
  const handleCompact = useCallback(() => {
    void runCompactWithFeedback();
  }, [runCompactWithFeedback]);
  const handleKeyDown = useComposerKeyboard({
    activeSuggestionIndex,
    addChip,
    atResults,
    chipsCount: chips.length,
    closeModelMenu,
    closeSuggestions,
    composingRef,
    onSend: handleSend,
    removeLastChip,
    setActiveSuggestionIndex,
    showSuggestions,
    valueRef,
  });

  return {
    handleChange,
    handleCompositionEnd,
    handleCompositionStart,
    handleKeyDown,
    handleResume,
    handleSend,
    handleCompact,
    handleStop,
    handleToggleModelMenu,
    isCompacting,
    isResuming,
    resumeError,
  };
}
