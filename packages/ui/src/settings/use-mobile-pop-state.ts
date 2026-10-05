import { useEffect } from "react";
import { useLatestRef } from "../core/use-latest-ref";

export type MobilePopStateHandler = (event: PopStateEvent) => void;

/** Registers a popstate listener only for touch settings hosts and keeps its callback current. */
export function useMobilePopState(mobile: boolean, onPopState: MobilePopStateHandler): void {
  const handler = useLatestRef(onPopState);

  useEffect(() => {
    if (!mobile || typeof window === "undefined") return;
    const handlePopState = (event: PopStateEvent) => handler.current(event);
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, [handler, mobile]);
}
