import type { PointerEvent } from "react";
import type { WindowResizeEdge } from "../keyboard/window-host";
import { windowResizeEdge } from "./window-resize";

export interface UseWindowResizeCaptureOptions {
  resizeWindow?: (edge: WindowResizeEdge) => Promise<void>;
  windowMaximized: boolean;
  setError: (error: string) => void;
}

/** Handles edge pointer presses for resizable settings windows. */
export function useWindowResizeCapture({
  resizeWindow,
  windowMaximized,
  setError,
}: UseWindowResizeCaptureOptions) {
  return (event: PointerEvent<HTMLDivElement>) => {
    if (!resizeWindow || event.button !== 0 || windowMaximized) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const edge = windowResizeEdge(event, rect);
    if (!edge) return;
    event.preventDefault();
    event.stopPropagation();
    void resizeWindow(edge).catch(() => setError("无法调整窗口大小，请重试。"));
  };
}
