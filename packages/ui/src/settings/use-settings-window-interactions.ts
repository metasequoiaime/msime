import { useEffect, useRef, type MouseEvent, type PointerEvent } from "react";
import type { WindowControl, WindowResizeEdge } from "../keyboard/window-host";
import { useWindowResizeCapture } from "./use-window-resize-capture";

export interface UseSettingsWindowInteractionsOptions {
  windowControl?: (action: WindowControl) => Promise<void>;
  beginWindowDrag?: () => Promise<void>;
  resizeWindow?: (edge: WindowResizeEdge) => Promise<void>;
  windowMaximized: boolean;
  macShell: boolean;
  onError: (message: string) => void;
}

/** Owns the shared settings shell's titlebar drag and edge-resize pointer behavior. */
export function useSettingsWindowInteractions({
  windowControl,
  beginWindowDrag,
  resizeWindow,
  windowMaximized,
  macShell,
  onError,
}: UseSettingsWindowInteractionsOptions) {
  const pendingDrag = useRef<{ x: number; y: number; pointerId: number } | null>(null);

  useEffect(() => {
    const clear = () => {
      pendingDrag.current = null;
    };
    window.addEventListener("blur", clear);
    return () => {
      clear();
      window.removeEventListener("blur", clear);
    };
  }, []);

  const resizeCapture = useWindowResizeCapture({
    resizeWindow: macShell ? undefined : resizeWindow,
    windowMaximized,
    setError: onError,
  });
  const onWindowPointerDownCapture = (event: PointerEvent<HTMLDivElement>) => {
    pendingDrag.current = null;
    resizeCapture(event);
  };

  const windowDragHandlers = {
    onDoubleClick: (event: MouseEvent<HTMLElement>) => {
      pendingDrag.current = null;
      if (event.button !== 0 || !windowControl) return;
      const shell = event.currentTarget.closest("[data-settings-shell]");
      if (!shell) return;
      const rect = shell.getBoundingClientRect();
      if (
        !windowMaximized &&
        resizeWindow &&
        (event.clientX - rect.left < 8 ||
          rect.right - event.clientX < 8 ||
          event.clientY - rect.top < 8 ||
          rect.bottom - event.clientY < 8)
      )
        return;
      void windowControl(windowMaximized ? "restore" : "maximize");
    },
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      if (event.button === 0 && event.detail < 2 && beginWindowDrag)
        pendingDrag.current = {
          x: event.clientX,
          y: event.clientY,
          pointerId: event.pointerId,
        };
    },
    onPointerMove: (event: PointerEvent<HTMLElement>) => {
      const pending = pendingDrag.current;
      if (!pending || pending.pointerId !== event.pointerId) return;
      if (event.buttons !== 1) {
        pendingDrag.current = null;
        return;
      }
      if (Math.abs(event.clientX - pending.x) + Math.abs(event.clientY - pending.y) < 2) return;
      pendingDrag.current = null;
      void (async () => {
        try {
          await beginWindowDrag?.();
        } catch {
          onError("无法移动窗口，请重试。");
        }
      })();
    },
    onPointerUp: () => {
      pendingDrag.current = null;
    },
    onPointerCancel: () => {
      pendingDrag.current = null;
    },
    onPointerLeave: () => {
      pendingDrag.current = null;
    },
  };

  const keepPointer = {
    onPointerDown: (event: PointerEvent<HTMLElement>) => event.stopPropagation(),
    onDoubleClick: (event: MouseEvent<HTMLElement>) => event.stopPropagation(),
  };

  return { onWindowPointerDownCapture, windowDragHandlers, keepPointer };
}
