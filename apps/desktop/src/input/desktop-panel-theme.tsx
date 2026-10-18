import type { ReactNode } from "react";
import type { SettingsClient } from "../../../../packages/ui/src/core/settings-client";
import { usePreferencesSnapshot } from "../../../../packages/ui/src/settings/use-preferences-snapshot";
import { useCandidatePreviewTheme } from "../../../../packages/ui/src/candidate/candidate-preview-theme";
import { panelSurfaceTheme, usePanelHost } from "./panel-host";

/** 三种桌面面板共用偏好订阅与宿主主题投影，入口只负责装配面板。 */
export function DesktopPanelTheme({
  preferences,
  surface,
  children,
}: {
  preferences: Pick<SettingsClient, "load" | "onPreferencesChanged" | "host">;
  surface: "handwriting" | "voice" | "emoji";
  children: (theme: "dark" | "light") => ReactNode;
}) {
  const snapshot = usePreferencesSnapshot(preferences);
  const host = usePanelHost(preferences.host);
  const surfaceTheme =
    surface === "handwriting"
      ? snapshot?.preferences.handwriting_theme
      : surface === "voice"
        ? snapshot?.preferences.voice_theme
        : snapshot?.preferences.emoji_theme;
  return children(
    useCandidatePreviewTheme(snapshot?.preferences.theme, panelSurfaceTheme(host, surfaceTheme)),
  );
}
