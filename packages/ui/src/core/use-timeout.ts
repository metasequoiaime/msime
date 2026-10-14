import { useEffect } from "react";
import { useLatestRef } from "./use-latest-ref";

/** Runs a callback after a delay and cancels it when the owner unmounts or changes. */
export function useTimeout(callback: () => void, delayMs: number | null): void {
  const callbackRef = useLatestRef(callback);
  useEffect(() => {
    if (delayMs === null) return;
    const timer = window.setTimeout(() => callbackRef.current(), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs]);
}
