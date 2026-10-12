import { useEffect, useRef } from "react";

/** Runs a callback after a delay and cancels it when the owner unmounts or changes. */
export function useTimeout(callback: () => void, delayMs: number | null): void {
  const callbackRef = useRef(callback);
  callbackRef.current = callback;
  useEffect(() => {
    if (delayMs === null) return;
    const timer = window.setTimeout(() => callbackRef.current(), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs]);
}
