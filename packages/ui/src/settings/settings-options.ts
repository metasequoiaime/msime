import type { HostCapabilities } from "../index";
import type { ThemeCatalogEntry } from "../theme/global-theme";
export { logo } from "./app-resources";
export {
  allTouchKeyboardSchemes,
  selectTouchKeyboardScheme,
  touchKeyboardSchemeOptions,
} from "./touch-keyboard-scheme-helpers";
export { defaultAiAssistant } from "./ai-assistant-defaults";
export { defaultVoiceInput } from "./voice-input-defaults";
export { defaultNavigation } from "./navigation-section";
export { localDictionaryKinds } from "../dictionary/dictionary-kinds";

// Options and defaults that the settings model in index.tsx shares with the settings pages, or that several pages share with each other.

// The Linux hosts do not draw the candidate list themselves; when the desktop panel that does ignores these settings, the host says why (HostCapabilities.candidate_panel_limit) and the appearance and skin pages say so once.
export const candidatePanelLimitNotes: Record<
  NonNullable<HostCapabilities["candidate_panel_limit"]>,
  string
> = {
  gnome_shell:
    "GNOME Shell 自己绘制 IBus 候选窗口并跟随 Shell 主题，这里的候选字体、颜色和皮肤在当前桌面不会生效。",
  fcitx_theme:
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，这里的候选颜色和皮肤不会覆盖它；字体仍然生效。把它改回 Fcitx5 默认主题，或在这里换一个全局主题，即可让候选颜色与皮肤生效。",
  kimpanel:
    "Fcitx5 的候选窗口由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色和皮肤不会生效。",
};

// The last column is the description on a host whose skin reaches only the candidate window (Linux presents the toolbar as an input method menu).
/** A theme card's one-line description. `candidateOnly` is for hosts without a floating toolbar. */
export function globalThemeDescription(entry: ThemeCatalogEntry, candidateOnly = false): string {
  if (entry.id === "system")
    return candidateOnly
      ? "候选窗口与键盘使用平台自带配色"
      : "候选窗口、悬浮工具栏与键盘使用平台自带配色";
  // 只在 iOS 提供，与 iOS 设置页 FeatureSettingsViews.swift 的说明一致。
  if (entry.id === "native") return "iOS 自带键盘的样子，跟随系统明暗，不跟季节";
  if (entry.id === "custom") return "外部皮肤、候选颜色与自定义键盘";
  const tone = entry.appearance === "dark" ? "深色" : "浅色";
  return candidateOnly ? `${tone}候选窗口与键盘` : `${tone}候选窗口、悬浮工具栏与键盘`;
}
