import { SettingsGroupNote } from "./settings-group-note";

export interface SkinPlatformNoticeProps {
  mobile: boolean;
  linux: boolean;
}

/** 主题页的总说明：各类宿主上主题作用于哪些表面。放在「明暗」组里，收进组名旁的「?」。 */
export function SkinPlatformNotice({ mobile, linux }: SkinPlatformNoticeProps) {
  const text = mobile
    ? "选择候选栏和键盘使用的主题；明暗预览仅影响当前卡片，不修改设置。"
    : linux
      ? "选择候选窗口使用的主题；明暗预览仅影响当前卡片，不修改设置。"
      : "选择候选窗口和悬浮工具栏使用的主题；明暗预览仅影响当前卡片，不修改设置。";

  return <SettingsGroupNote>{text}</SettingsGroupNote>;
}
