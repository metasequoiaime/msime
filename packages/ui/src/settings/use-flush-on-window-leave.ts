import { useEffect } from "react";
import { useLatestRef } from "../core/use-latest-ref";

export type FlushOnWindowLeave = () => void | Promise<void>;

/** Flushes pending edits when the settings surface is blurred, hidden, or backgrounded. */
export function useFlushOnWindowLeave(flush: FlushOnWindowLeave): void {
  const flushRef = useLatestRef(flush);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    const flushNow = () => void flushRef.current();
    const onVisibilityChange = () => {
      if (document.hidden) flushNow();
    };
    window.addEventListener("blur", flushNow);
    window.addEventListener("pagehide", flushNow);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.removeEventListener("blur", flushNow);
      window.removeEventListener("pagehide", flushNow);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, []);
}
