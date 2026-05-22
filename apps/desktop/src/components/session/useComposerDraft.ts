import { useCallback, useEffect, useRef, useState } from "react";

const COMPOSER_MAX_INPUT_HEIGHT = 128;

type ComposerDraftValue = string | ((current: string) => string);

interface UseComposerDraftOptions {
  pendingInput: string;
  setPendingInput: (input: string) => void;
}

export function useComposerDraft({ pendingInput, setPendingInput }: UseComposerDraftOptions) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const composingRef = useRef(false);
  const valueRef = useRef("");
  const [value, setValueState] = useState("");

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const nextHeight = Math.min(el.scrollHeight, COMPOSER_MAX_INPUT_HEIGHT);
    el.style.height = `${nextHeight}px`;
    el.style.overflowY = el.scrollHeight > COMPOSER_MAX_INPUT_HEIGHT ? "auto" : "hidden";
  }, []);

  const focusTextarea = useCallback(() => {
    textareaRef.current?.focus();
  }, []);

  const setValue = useCallback((nextValue: ComposerDraftValue) => {
    setValueState((current) => {
      const next = typeof nextValue === "function" ? nextValue(current) : nextValue;
      valueRef.current = next;
      return next;
    });
  }, []);

  const resetDraft = useCallback(() => {
    valueRef.current = "";
    setValueState("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.overflowY = "hidden";
    }
  }, []);

  useEffect(() => {
    if (!pendingInput) return;

    setValue((current) => current.trim()
      ? `${current.trimEnd()}\n\n${pendingInput}`
      : pendingInput);
    setPendingInput("");
    setTimeout(() => {
      focusTextarea();
      adjustHeight();
    }, 0);
  }, [adjustHeight, focusTextarea, pendingInput, setPendingInput, setValue]);

  return {
    adjustHeight,
    composingRef,
    focusTextarea,
    resetDraft,
    setValue,
    textareaRef,
    value,
    valueRef,
  };
}
