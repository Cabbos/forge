import { useCallback, useState, type MutableRefObject, type RefObject } from "react";
import type { ComposerChip } from "./composerTypes";

type SetComposerValue = (nextValue: string | ((current: string) => string)) => void;

interface UseComposerChipsOptions {
  closeSuggestions: () => void;
  focusTextarea: () => void;
  setValue: SetComposerValue;
  textareaRef: RefObject<HTMLTextAreaElement>;
  valueRef: MutableRefObject<string>;
}

export function useComposerChips({
  closeSuggestions,
  focusTextarea,
  setValue,
  textareaRef,
  valueRef,
}: UseComposerChipsOptions) {
  const [chips, setChips] = useState<ComposerChip[]>([]);

  const clearChips = useCallback(() => {
    setChips([]);
  }, []);

  const removeChip = useCallback((id: string) => {
    setChips((current) => current.filter((chip) => chip.id !== id));
  }, []);

  const removeLastChip = useCallback(() => {
    setChips((current) => current.slice(0, -1));
  }, []);

  const addChip = useCallback((type: ComposerChip["type"], value: string) => {
    setChips((current) => current.some((chip) => chip.value === value)
      ? current
      : [...current, { id: crypto.randomUUID(), type, value }]);
    closeSuggestions();
    removeTriggerTextForChip(type, textareaRef, valueRef, setValue);
    setTimeout(focusTextarea, 0);
  }, [closeSuggestions, focusTextarea, setValue, textareaRef, valueRef]);

  return {
    addChip,
    chips,
    clearChips,
    removeChip,
    removeLastChip,
  };
}

function removeTriggerTextForChip(
  type: ComposerChip["type"],
  textareaRef: RefObject<HTMLTextAreaElement>,
  valueRef: MutableRefObject<string>,
  setValue: SetComposerValue,
) {
  setValue((current) => {
    const position = textareaRef.current?.selectionStart ?? current.length;
    const before = current.slice(0, position);
    const after = current.slice(position);
    const trigger = type === "file" ? "@" : "/";
    const triggerIndex = before.lastIndexOf(trigger);
    const next = triggerIndex >= 0 ? before.slice(0, triggerIndex) + after : current;
    valueRef.current = next;
    return next;
  });
}
