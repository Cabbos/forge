import { useEffect } from "react";
import type { RefObject } from "react";

interface UseComposerMenuDismissalOptions<T extends HTMLElement> {
  isMenuOpen: boolean;
  onDismiss: () => void;
  rootRef: RefObject<T | null>;
}

export function useComposerMenuDismissal<T extends HTMLElement>({
  isMenuOpen,
  onDismiss,
  rootRef,
}: UseComposerMenuDismissalOptions<T>) {
  useEffect(() => {
    if (!isMenuOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && rootRef.current?.contains(target)) return;
      onDismiss();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [isMenuOpen, onDismiss, rootRef]);
}
