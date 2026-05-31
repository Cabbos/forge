import { useCallback, type ChangeEvent, type MutableRefObject } from "react";

type ComposerDraftSetter = (nextValue: string | ((current: string) => string)) => void;

interface UseComposerInputHandlersOptions {
  adjustHeight: () => void;
  closeSuggestions: () => void;
  composingRef: MutableRefObject<boolean>;
  sessionId: string;
  setValue: ComposerDraftSetter;
  stop: (sessionId: string) => void;
  syncSuggestionsForInput: (inputValue: string, cursorPosition: number) => void;
  toggleModelMenu: () => void;
  valueRef: MutableRefObject<string>;
}

export function useComposerInputHandlers({
  adjustHeight,
  closeSuggestions,
  composingRef,
  sessionId,
  setValue,
  stop,
  syncSuggestionsForInput,
  toggleModelMenu,
  valueRef,
}: UseComposerInputHandlersOptions) {
  const handleChange = useCallback((event: ChangeEvent<HTMLTextAreaElement>) => {
    const nextValue = event.target.value;
    valueRef.current = nextValue;
    setValue(nextValue);
    adjustHeight();
    syncSuggestionsForInput(nextValue, event.target.selectionStart);
  }, [adjustHeight, setValue, syncSuggestionsForInput, valueRef]);

  const handleToggleModelMenu = useCallback(() => {
    closeSuggestions();
    toggleModelMenu();
  }, [closeSuggestions, toggleModelMenu]);

  const handleStop = useCallback(() => {
    stop(sessionId);
  }, [sessionId, stop]);

  const handleCompositionStart = useCallback(() => {
    composingRef.current = true;
  }, [composingRef]);

  const handleCompositionEnd = useCallback(() => {
    composingRef.current = false;
    adjustHeight();
  }, [adjustHeight, composingRef]);

  return {
    handleChange,
    handleCompositionEnd,
    handleCompositionStart,
    handleStop,
    handleToggleModelMenu,
  };
}
