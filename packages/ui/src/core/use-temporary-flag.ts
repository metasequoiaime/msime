import { useCallback, useEffect, useRef, useState } from "react";

/** A transient UI flag whose deadline restarts whenever it is shown again. */
export function useTemporaryFlag(durationMs: number) {
  const [active, setActive] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const mounted = useRef(false);

  const show = useCallback(() => {
    if (!mounted.current) return;
    clearTimeout(timer.current);
    setActive(true);
    timer.current = setTimeout(() => setActive(false), durationMs);
  }, [durationMs]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      clearTimeout(timer.current);
    };
  }, []);

  return [active, show] as const;
}
