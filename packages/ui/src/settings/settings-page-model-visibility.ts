import type { SettingsPageId } from "./settings-page-registry";
import type { SettingsPlatform } from "../theme/platform-tokens";

/** 触屏设置列表不提供实体键盘快捷键、悬浮工具栏和桌面插件页。 */
export const mobileHiddenPageIds: readonly SettingsPageId[] = [
  "shortcuts",
  "floating-toolbar",
  "plugins",
];

/** 宿主报告 `touch_plugin_packs` 时，触屏设置列表照样列出插件页。模块级常量，每次渲染拿到同一个数组。 */
const touchPluginHiddenPageIds: readonly SettingsPageId[] = mobileHiddenPageIds.filter(
  (id) => id !== "plugins",
);

/** 触屏设置列表不列出的页面：宿主有触屏插件页时不藏「插件」。 */
export function mobileHiddenPages(touchPluginPacks: boolean): readonly SettingsPageId[] {
  return touchPluginPacks ? touchPluginHiddenPageIds : mobileHiddenPageIds;
}

export interface SettingsPageModelVisibilityOptions {
  mobilePlatform: boolean;
  settingsPlatform: SettingsPlatform;
  hasHost: boolean;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  macosPlatform: boolean;
  showRestartInputMethod: boolean;
  hasMcpServerStatus: boolean;
  showHelpcodeShiftEntry: boolean;
  editionUsesHelpcode: boolean;
}

/** 集中计算设置模型中与平台和页面可见性有关的纯判定。 */
export function settingsPageModelVisibility({
  mobilePlatform,
  settingsPlatform,
  hasHost,
  linuxPlatform,
  windowsPlatform,
  macosPlatform,
  showRestartInputMethod,
  hasMcpServerStatus,
  showHelpcodeShiftEntry,
  editionUsesHelpcode,
}: SettingsPageModelVisibilityOptions) {
  const showHelpcode = (!mobilePlatform || showHelpcodeShiftEntry) && editionUsesHelpcode;
  const showDeveloperPage =
    hasMcpServerStatus ||
    showRestartInputMethod ||
    !hasHost ||
    linuxPlatform ||
    windowsPlatform ||
    macosPlatform;
  const untitledOnPhone: readonly SettingsPageId[] =
    settingsPlatform === "harmony" ? [] : ["home", "typing-statistics", "account"];

  return { showHelpcode, showDeveloperPage, untitledOnPhone } as const;
}
