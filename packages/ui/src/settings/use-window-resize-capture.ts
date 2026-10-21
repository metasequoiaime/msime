import type { PointerEvent } from "react";
import type { WindowResizeEdge } from "../keyboard/window-host";
import { windowResizeEdge } from "./window-resize";

export interface UseWindowResizeCaptureOptions {
  resizeWindow?: (edge: WindowResizeEdge) => Promise<void>;
  windowMaximized: boolean;
  setError: (error: string) => void;
}

/** 可调整大小的设置窗口和共享面板复用边缘拖动判定。 */
export function useWindowResizeCapture({
  resizeWindow,
  windowMaximized,
  setError,
}: UseWindowResizeCaptureOptions) {
  return (event: PointerEvent<HTMLElement>) => {
    if (!resizeWindow || event.button !== 0 || windowMaximized) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const edge = windowResizeEdge(event, rect);
    if (!edge) return;
    event.preventDefault();
    event.stopPropagation();
    void resizeWindow(edge).catch(() => setError("无法调整窗口大小，请重试。"));
  };
}
