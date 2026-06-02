import { useCallback, useRef, useState } from "react";

export function useWikiBusyState() {
  const [busyId, setBusyId] = useState<string | null>(null);
  const busyTokenRef = useRef(0);

  const beginBusy = useCallback((id: string) => {
    const token = busyTokenRef.current + 1;
    busyTokenRef.current = token;
    setBusyId(id);
    return token;
  }, []);

  const clearBusy = useCallback((token: number, id: string) => {
    if (busyTokenRef.current !== token) return;
    setBusyId((current) => (current === id ? null : current));
  }, []);

  return { busyId, setBusyId, beginBusy, clearBusy };
}
