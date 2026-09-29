import { useLayoutEffect, useRef } from "react";

/** Returns the shared settings surface ref and resets it when the visible page changes. */
export function useSettingsContentScrollReset(page: unknown) {
  const settingsContentRef = useRef<HTMLElement>(null);

  useLayoutEffect(() => {
    if (settingsContentRef.current) settingsContentRef.current.scrollTop = 0;
  }, [page]);

  return settingsContentRef;
}
