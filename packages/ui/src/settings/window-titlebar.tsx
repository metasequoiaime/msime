import { useEffect, useRef } from "react";
import type { WindowControl, WindowResizeEdge } from "../keyboard/window-host";
import { windowIcons } from "./app-resources";
import * as settings from "./settings-style";

export interface WindowTitlebarProps {
  maximized: boolean;
  windowControl?: (action: WindowControl) => Promise<void>;
  beginWindowDrag?: () => Promise<void>;
  resizeWindow?: (edge: WindowResizeEdge) => Promise<void>;
  onError: (message: string) => void;
}

/** Desktop window titlebar controls and drag behavior shared by settings hosts. */
export function WindowTitlebar({
  maximized,
  windowControl,
  beginWindowDrag,
  resizeWindow,
  onError,
}: WindowTitlebarProps) {
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

  if (!windowControl && !beginWindowDrag) return null;

  return (
    <header
      className={settings.titlebar}
      aria-label="窗口控制"
      onDoubleClick={(event) => {
        pendingDrag.current = null;
        if (event.button !== 0 || !windowControl) return;
        const rect = event.currentTarget.parentElement!.getBoundingClientRect();
        if (
          !maximized &&
          resizeWindow &&
          (event.clientX - rect.left < 8 ||
            rect.right - event.clientX < 8 ||
            event.clientY - rect.top < 8 ||
            rect.bottom - event.clientY < 8)
        )
          return;
        void windowControl(maximized ? "restore" : "maximize");
      }}
      onPointerDown={(event) => {
        if (event.button === 0 && event.detail < 2 && beginWindowDrag)
          pendingDrag.current = {
            x: event.clientX,
            y: event.clientY,
            pointerId: event.pointerId,
          };
      }}
      onPointerMove={(event) => {
        const pending = pendingDrag.current;
        if (!pending || pending.pointerId !== event.pointerId) return;
        if (event.buttons !== 1) {
          pendingDrag.current = null;
          return;
        }
        if (Math.abs(event.clientX - pending.x) + Math.abs(event.clientY - pending.y) < 2) return;
        pendingDrag.current = null;
        // Invoke during the gesture; catch synchronous and asynchronous host failures.
        void (async () => {
          try {
            await beginWindowDrag?.();
          } catch {
            onError("无法移动窗口，请重试。");
          }
        })();
      }}
      onPointerUp={() => {
        pendingDrag.current = null;
      }}
      onPointerCancel={() => {
        pendingDrag.current = null;
      }}
      onPointerLeave={() => {
        pendingDrag.current = null;
      }}
    >
      <span className={settings.title} data-window-title="">
        水杉 IME
      </span>
      {windowControl && (
        <span
          className={settings.windowControls}
          onPointerDown={(event) => event.stopPropagation()}
          onDoubleClick={(event) => event.stopPropagation()}
        >
          <button type="button" aria-label="最小化" onClick={() => void windowControl("minimize")}>
            <img
              className={settings.windowIcon}
              src={windowIcons.minimize}
              alt=""
              draggable={false}
            />
          </button>
          <button
            type="button"
            aria-label={maximized ? "还原" : "最大化"}
            onClick={() => void windowControl(maximized ? "restore" : "maximize")}
          >
            <img
              className={settings.windowIcon}
              src={maximized ? windowIcons.restore : windowIcons.maximize}
              alt=""
              draggable={false}
            />
          </button>
          <button
            type="button"
            className={settings.windowClose}
            aria-label="关闭"
            onClick={() => void windowControl("close")}
          >
            <img className={settings.windowIcon} src={windowIcons.close} alt="" draggable={false} />
          </button>
        </span>
      )}
    </header>
  );
}
