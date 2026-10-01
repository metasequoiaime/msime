import { useEffect, useState } from "react";
import {
  KeyboardPanel,
  usePreferencesSnapshot,
  type PanelClient,
  type SettingsClient,
  type TouchKeyboardSkinDesign,
  defaultTouchKeyboardGeometry,
  keyboardThemeId,
} from "@msime/ui";
import { useCandidatePreviewTheme } from "../../../../packages/ui/src/candidate/candidate-preview-theme";
import { invoke, isTauri } from "@tauri-apps/api/core";

type ThemeClient = Pick<SettingsClient, "load" | "onPreferencesChanged" | "host">;

/** The host platform a panel window runs under: the one the client already knows, or else the one the shell reports. */
export function useHostPlatform(host: SettingsClient["host"]): string | undefined {
  const [platform, setPlatform] = useState<string | undefined>(host?.platform);
  useEffect(() => {
    let active = true;
    if (host?.platform) setPlatform(host.platform);
    else if (isTauri())
      void invoke<{ platform: string }>("host_capabilities")
        .then((reported) => {
          if (active) setPlatform(reported.platform);
        })
        .catch(() => {});
    return () => {
      active = false;
    };
  }, [host?.platform]);
  return platform;
}

export function DesktopKeyboard({
  client,
  preferences,
}: {
  client: PanelClient;
  preferences: ThemeClient;
}) {
  const platform = useHostPlatform(preferences.host);
  const snapshot = usePreferencesSnapshot(preferences, true);
  const theme = useCandidatePreviewTheme(
    snapshot?.preferences.theme,
    snapshot?.preferences.screen_keyboard_theme,
  );
  const layout =
    snapshot?.preferences.touch_keyboard_layout === "nine_key" ? "nine_key" : "twenty_six_key";
  const keySpacingTenths =
    snapshot?.preferences.touch_key_spacing_tenths ?? defaultTouchKeyboardGeometry.keySpacingTenths;
  const rowSpacingTenths =
    snapshot?.preferences.touch_row_spacing_tenths ?? defaultTouchKeyboardGeometry.rowSpacingTenths;
  const skin = keyboardThemeId(
    snapshot?.preferences.global_theme,
    snapshot?.preferences.custom_theme,
  );
  const customDesign = snapshot?.preferences.custom_theme?.keyboard ?? undefined;
  // macOS voice submission belongs to the native IMK session. A standalone
  // Tauri keyboard does not own that session, so exposing this button would
  // only lead to an unusable voice panel; the native input-method toolbar and
  // shortcut remain the supported entry points.
  const voiceShortcut =
    platform !== undefined &&
    platform !== "macos" &&
    snapshot?.preferences.touch_voice_shortcut === true;
  return (
    <KeyboardPanel
      client={client}
      platform={platform}
      theme={theme}
      layout={layout}
      keySpacingTenths={keySpacingTenths}
      rowSpacingTenths={rowSpacingTenths}
      voiceShortcut={voiceShortcut}
      skin={skin}
      customDesign={customDesign as TouchKeyboardSkinDesign | undefined}
    />
  );
}
