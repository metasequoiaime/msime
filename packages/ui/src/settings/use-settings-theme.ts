import { useEffect } from "react";
import { resolveSettingsTheme } from "./theme-helpers";
import type { SurfaceTheme, ThemeMode } from "./theme-settings-section";

/** Synchronizes the settings document theme and follows the system palette when requested. */
export function useSettingsTheme(themeMode: ThemeMode, settingsTheme: SurfaceTheme) {
  useEffect(() => {
    if (typeof document === "undefined") return;
    const apply = () => {
      document.documentElement.dataset.theme = resolveSettingsTheme(themeMode, settingsTheme);
    };
    apply();
    if (
      themeMode !== "system" ||
      settingsTheme !== "follow" ||
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    )
      return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const listener = () => apply();
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", listener);
      return () => media.removeEventListener("change", listener);
    }
    media.addListener(listener);
    return () => media.removeListener(listener);
  }, [settingsTheme, themeMode]);
}
