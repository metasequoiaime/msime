import type { SurfaceTheme, ThemeMode } from "./theme-settings-section";

export function resolveSettingsTheme(theme: ThemeMode, surface: SurfaceTheme): "dark" | "light" {
  if (surface !== "follow") return surface;
  if (theme !== "system") return theme;
  return typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}
