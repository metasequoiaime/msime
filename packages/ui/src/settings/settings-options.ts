import type { HostCapabilities } from "../core/host-capabilities";

import type { ThemeCatalogEntry } from "../theme/global-theme";
export { logo } from "./app-resources";
export {
  allTouchKeyboardSchemes,
  selectTouchKeyboardScheme,
  touchKeyboardSchemeOptions,
} from "./touch-keyboard-scheme-helpers";
export { defaultAiAssistant } from "./ai-assistant-defaults";
export { defaultVoiceInput } from "./voice-input-defaults";
export { defaultNavigation } from "./candidate-paging-preferences";
export { localDictionaryKinds } from "../dictionary/dictionary-kinds";

// Options and defaults that the settings model in index.tsx shares with the settings pages, or that several pages share with each other.

// The Linux hosts do not draw the candidate list themselves; when the desktop panel that does ignores these settings, the host says why (HostCapabilities.candidate_panel_limit) and the appearance and skin pages say so once.
export const candidatePanelLimitNotes: Record<
  NonNullable<HostCapabilities["candidate_panel_limit"]>,
  string
> = {
  gnome_shell:
    "GNOME Shell 自己绘制 IBus 候选窗口并跟随 Shell 主题，这里的候选字体、颜色、皮肤、整体大小、圆角、内边距、不透明度和水杉 logo 在当前桌面都不会生效。",
  fcitx_theme:
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，或者设置了 MSIME_FCITX5_THEME_AUTOGENERATE=0 停止更新水杉主题，这里的候选颜色和皮肤不会写进候选窗，圆角、内边距、不透明度和水杉 logo 也不会生效，整体大小只缩放字号；字体仍然生效。把主题改回 Fcitx5 默认主题、在这里换一个全局主题，或去掉这个环境变量后重启 Fcitx5，即可让这些设置生效。",
  kimpanel:
    "Fcitx5 的候选窗口由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色、皮肤、整体大小、圆角、内边距、不透明度和水杉 logo 都不会生效。",
  ibus_panel:
    "IBus 的候选窗口由桌面的输入法面板绘制，只接收候选字体和颜色：整体大小只缩放字号，这里的圆角、内边距、不透明度、水杉 logo 和皮肤的装饰图在 IBus 下不会生效。",
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
