import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties, WheelEvent } from "react";
import type { BlockState } from "@/lib/protocol";

interface ConversationScrollOptions {
  blockCount: number;
  lastBlock?: BlockState;
}

export const BOTTOM_LOCK_THRESHOLD = 96;

export function useConversationScroll({ blockCount, lastBlock }: ConversationScrollOptions) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const scrollRafRef = useRef<number | null>(null);
  const autoScrollRafRef = useRef<number | null>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  const setScrolledUpIfChanged = useCallback((next: boolean) => {
    setUserScrolledUp((current) => (current === next ? current : next));
  }, []);

  const cancelAutoScroll = useCallback(() => {
    if (autoScrollRafRef.current === null) return;
    cancelAnimationFrame(autoScrollRafRef.current);
    autoScrollRafRef.current = null;
  }, []);

  const updateStickiness = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    const isAtBottom = distanceFromBottom <= BOTTOM_LOCK_THRESHOLD;
    stickToBottomRef.current = isAtBottom;
    setScrolledUpIfChanged(!isAtBottom);
  }, [setScrolledUpIfChanged]);

  useEffect(() => {
    if (!stickToBottomRef.current) return;
    if (autoScrollRafRef.current !== null) {
      cancelAnimationFrame(autoScrollRafRef.current);
    }
    autoScrollRafRef.current = requestAnimationFrame(() => {
      autoScrollRafRef.current = null;
      const el = scrollRef.current;
      if (!el) return;
      el.scrollTop = el.scrollHeight;
      setScrolledUpIfChanged(false);
    });
    return () => {
      if (autoScrollRafRef.current !== null) {
        cancelAnimationFrame(autoScrollRafRef.current);
        autoScrollRafRef.current = null;
      }
    };
  }, [blockCount, lastBlock?.content, lastBlock?.isComplete, setScrolledUpIfChanged]);

  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
      if (autoScrollRafRef.current !== null) {
        cancelAnimationFrame(autoScrollRafRef.current);
      }
    };
  }, []);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight > BOTTOM_LOCK_THRESHOLD) {
      cancelAutoScroll();
    }
    if (scrollRafRef.current !== null) return;
    scrollRafRef.current = requestAnimationFrame(() => {
      scrollRafRef.current = null;
      updateStickiness();
    });
  }, [cancelAutoScroll, updateStickiness]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      cancelAutoScroll();
      stickToBottomRef.current = false;
      setScrolledUpIfChanged(true);
    }
  }, [cancelAutoScroll, setScrolledUpIfChanged]);

  const scrollToBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    stickToBottomRef.current = true;
    el.scrollTop = el.scrollHeight;
    setScrolledUpIfChanged(false);
  }, [setScrolledUpIfChanged]);

  const scrollStyle: CSSProperties = {
    scrollbarGutter: "stable",
    overflowAnchor: userScrolledUp ? "auto" : "none",
  };

  return {
    scrollRef,
    userScrolledUp,
    handleScroll,
    handleWheel,
    scrollToBottom,
    scrollStyle,
  };
}
