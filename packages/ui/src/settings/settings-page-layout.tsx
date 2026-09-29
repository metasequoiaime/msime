import type { PointerEventHandler, ReactNode, RefObject } from "react";
import {
  SettingsNavigationChrome,
  type SettingsNavigationChromeProps,
} from "./settings-navigation-chrome";
import { WindowTitlebar, type WindowTitlebarProps } from "./window-titlebar";
import * as settings from "./settings-style";

export interface SettingsPageLayoutProps {
  mobile: boolean;
  confirmation: ReactNode;
  onPointerDownCapture: PointerEventHandler<HTMLDivElement>;
  windowTitlebar: WindowTitlebarProps;
  navigation: SettingsNavigationChromeProps;
  contentRef: RefObject<HTMLElement | null>;
  children: ReactNode;
}

/** Shared settings shell with desktop window chrome, responsive navigation and scroll surface. */
export function SettingsPageLayout({
  mobile,
  confirmation,
  onPointerDownCapture,
  windowTitlebar,
  navigation,
  contentRef,
  children,
}: SettingsPageLayoutProps) {
  return (
    <div
      className={settings.shell}
      data-settings-shell=""
      // The phone hosts read as one product with the Apple app, which is where the palette below
      // comes from. The inherited one is the Windows settings accent.
      data-mobile={mobile ? "" : undefined}
      onPointerDownCapture={onPointerDownCapture}
    >
      {confirmation}
      {/* A phone has no window to minimise, maximise, close or drag: the OS owns the frame. The host
          still exposes the window commands on mobile because the same Tauri app binary backs both, so
          the presence of a command is not the question -- the platform is. */}
      {!mobile && <WindowTitlebar {...windowTitlebar} />}
      <div
        className="flex min-h-0 min-w-0 flex-1 overflow-hidden max-phone:flex-col"
        data-settings-body=""
      >
        {/* A bottom tab bar. `order-2` seats it below the content while the DOM keeps it ahead, so
            assistive technology and keyboard focus still reach the navigation first, and the bottom
            padding clears the gesture inset. Hidden above phone width, where the sidebar serves. */}
        <SettingsNavigationChrome {...navigation} />
        <main
          ref={contentRef}
          id="settings-content"
          className="min-h-0 min-w-0 flex-1 overflow-y-auto pt-0 pr-6 pb-0 pl-4 [scrollbar-gutter:stable] max-phone:px-2"
          aria-labelledby="page-title"
        >
          <div className="mx-auto mt-0.5 mb-0 w-full max-w-[900px] p-3 max-phone:px-1 max-phone:py-3">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
