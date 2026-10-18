import { useEffect, useState } from "react";
import {
  KeyboardPanel,
  usePreferencesSnapshot,
  type PanelClient,
  type SettingsClient,
  type TouchKeyboardSkinDesign,
  keyboardThemeId,
  type NineKeyShuangpinKeyFace,
} from "@msime/ui";
import { useCandidatePreviewTheme } from "../../../../packages/ui/src/candidate/candidate-preview-theme";
import { touchKeyboardGeometryPreferences } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-preferences";
import { defaultTouchKeyboardGeometry } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-defaults";
import { panelSurfaceTheme, usePanelHost } from "./panel-host";
type ThemeClient = Pick<
  SettingsClient,
  "load" | "onPreferencesChanged" | "host" | "nineKeyShuangpinLayout"
>;

export { panelSurfaceTheme, useHostPlatform, usePanelHost } from "./panel-host";
export type { PanelHost, PanelHostState } from "./panel-host";

export function DesktopKeyboard({
  client,
  preferences,
}: {
  client: PanelClient;
  preferences: ThemeClient;
}) {
  const panelHostState = usePanelHost(preferences.host);
  const { host: panelHost, resolved: hostResolved } = panelHostState;
  const platform = panelHost?.platform;
  const snapshot = usePreferencesSnapshot(preferences, true);
  const [nineKeyShuangpinKeys, setNineKeyShuangpinKeys] = useState<
    readonly NineKeyShuangpinKeyFace[]
  >([]);
  useEffect(() => {
    if (snapshot?.preferences.touch_keyboard_layout !== "nine_key_shuangpin") return;
    const profile = snapshot.preferences.nine_key_shuangpin_profile ?? "migala3";
    const table = snapshot.preferences.nine_key_shuangpin_custom_profile;
    const reader = preferences.nineKeyShuangpinLayout;
    if (!reader) return;
    let active = true;
    void reader({ profile, table }).then((value) => {
      if (active) setNineKeyShuangpinKeys(value?.keys ?? []);
    });
    return () => {
      active = false;
    };
  }, [preferences, snapshot?.preferences]);
  const theme = useCandidatePreviewTheme(
    snapshot?.preferences.theme,
    panelSurfaceTheme(panelHostState, snapshot?.preferences.screen_keyboard_theme),
  );
  const layout =
    snapshot?.preferences.touch_keyboard_layout === "nine_key_shuangpin"
      ? "nine_key_shuangpin"
      : snapshot?.preferences.touch_keyboard_layout === "nine_key"
        ? "nine_key"
        : "twenty_six_key";
  // 宿主声明间距不可调（macOS）时，面板固定用默认间距，不读存下的值：设置页在那里不给间距滑块，存下的旧值不能继续生效。宿主能力还没读到时也先按默认间距画，否则偏好先到的话 macOS 上会先画出存下的旧间距、再跳回默认。
  const { keySpacingTenths, rowSpacingTenths } = touchKeyboardGeometryPreferences(
    !hostResolved || panelHost?.touch_keyboard_spacing === false
      ? {
          touch_key_spacing_tenths: defaultTouchKeyboardGeometry.keySpacingTenths,
          touch_row_spacing_tenths: defaultTouchKeyboardGeometry.rowSpacingTenths,
        }
      : snapshot?.preferences,
  );
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
      nineKeyShuangpinKeys={nineKeyShuangpinKeys}
      keySpacingTenths={keySpacingTenths}
      rowSpacingTenths={rowSpacingTenths}
      voiceShortcut={voiceShortcut}
      skin={skin}
      customDesign={customDesign as TouchKeyboardSkinDesign | undefined}
    />
  );
}
