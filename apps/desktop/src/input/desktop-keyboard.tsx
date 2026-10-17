import { useEffect, useState } from "react";
import {
  KeyboardPanel,
  usePreferencesSnapshot,
  type PanelClient,
  type SettingsClient,
  type SurfaceTheme,
  type TouchKeyboardSkinDesign,
  keyboardThemeId,
  type NineKeyShuangpinKeyFace,
} from "@msime/ui";
import { useCandidatePreviewTheme } from "../../../../packages/ui/src/candidate/candidate-preview-theme";
import { touchKeyboardGeometryPreferences } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-preferences";
import { defaultTouchKeyboardGeometry } from "../../../../packages/ui/src/settings/touch-keyboard-geometry-defaults";
import { invoke, isTauri } from "@tauri-apps/api/core";

type ThemeClient = Pick<
  SettingsClient,
  "load" | "onPreferencesChanged" | "host" | "nineKeyShuangpinLayout"
>;

/** 面板窗口读到的宿主能力：客户端已经知道的那份，否则是外壳报告的那份。键盘面板要用其中的平台和 `touch_keyboard_spacing`，各面板还要用 `surface_themes_follow` 决定读不读自己的单独明暗。 */
type PanelHost = {
  platform: string;
  touch_keyboard_spacing?: boolean;
  surface_themes_follow?: boolean;
};

/** `resolved` 为 false 表示还在等外壳报告宿主能力；报告失败或不在 Tauri 里时 `host` 为 undefined、`resolved` 为 true。 */
export type PanelHostState = { host?: PanelHost; resolved: boolean };

export function usePanelHost(host: SettingsClient["host"]): PanelHostState {
  const [state, setState] = useState<PanelHostState>({
    host,
    resolved: Boolean(host?.platform),
  });
  useEffect(() => {
    let active = true;
    if (host?.platform)
      setState((current) =>
        current.host === host && current.resolved ? current : { host, resolved: true },
      );
    else if (isTauri())
      void invoke<PanelHost>("host_capabilities")
        .then((value) => {
          if (active) setState({ host: value, resolved: true });
        })
        .catch(() => {
          if (active) setState({ host: undefined, resolved: true });
        });
    else
      setState((current) =>
        current.host === undefined && current.resolved
          ? current
          : { host: undefined, resolved: true },
      );
    return () => {
      active = false;
    };
  }, [host]);
  return state;
}

/** The host platform a panel window runs under: the one the client already knows, or else the one the shell reports. */
export function useHostPlatform(host: SettingsClient["host"]): string | undefined {
  return usePanelHost(host).host?.platform;
}

/** 面板实际按哪一项单独明暗着色：各界面一律跟随颜色模式的宿主（`surface_themes_follow`，macOS、Windows 和 Linux）不读存下的值，与原生面板一致。宿主能力还没读到时也先只按颜色模式画，免得面板在答复到来前闪一下已经砍掉的单独明暗；确定没有宿主（浏览器预览，或外壳没答上来）时照旧套用。 */
export function panelSurfaceTheme(
  { host, resolved }: PanelHostState,
  surface: SurfaceTheme | undefined,
): SurfaceTheme | undefined {
  return !resolved || host?.surface_themes_follow === true ? undefined : surface;
}

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
