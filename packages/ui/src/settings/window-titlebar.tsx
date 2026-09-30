import type { ReactNode } from "react";
import type { WindowControl } from "../keyboard/window-host";
import { windowIcons } from "./app-resources";
import * as settings from "./settings-style";
import type { useSettingsWindowInteractions } from "./use-settings-window-interactions";

type WindowInteractions = ReturnType<typeof useSettingsWindowInteractions>;

export interface WindowTitlebarProps {
  linux: boolean;
  logo: string;
  pageTitle: string;
  search?: ReactNode;
  maximized: boolean;
  windowControl?: (action: WindowControl) => Promise<void>;
  dragHandlers: WindowInteractions["windowDragHandlers"];
  keepPointer: WindowInteractions["keepPointer"];
}

/** Windows and Linux settings caption, using the shell's shared window gestures. */
export function WindowTitlebar({
  linux,
  logo,
  pageTitle,
  search,
  maximized,
  windowControl,
  dragHandlers,
  keepPointer,
}: WindowTitlebarProps) {
  return (
    <header className={settings.titlebar} aria-label="窗口控制" {...dragHandlers}>
      <span className={settings.titlebarBrand}>
        {!linux && <img src={logo} alt="" draggable={false} />}
        <span className={settings.title} data-window-title="">
          水杉输入法
        </span>
        {!linux && <span className={settings.titlebarSubtitle}>设置</span>}
      </span>
      {linux && (
        <span className={settings.titlebarPageTitle} aria-hidden="true">
          {pageTitle}
        </span>
      )}
      {search && (
        <label className={settings.titlebarSearch} {...keepPointer}>
          {search}
        </label>
      )}
      {windowControl && (
        <span className={settings.windowControls} {...keepPointer}>
          <button type="button" aria-label="最小化" onClick={() => void windowControl("minimize")}>
            <img className={settings.windowIcon} src={windowIcons.minimize} alt="" draggable={false} />
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
