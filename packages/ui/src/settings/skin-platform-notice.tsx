import { PageIntro } from "../core/platform-controls";

export interface SkinPlatformNoticeProps {
  mobile: boolean;
  linux: boolean;
}

/** 主题页的页首说明：各类宿主上主题作用于哪些表面。 */
export function SkinPlatformNotice({ mobile, linux }: SkinPlatformNoticeProps) {
  const text = mobile
    ? "选择候选栏和键盘使用的主题；明暗预览仅影响当前卡片，不修改设置。"
    : linux
      ? "选择候选窗使用的主题；明暗预览仅影响当前卡片，不修改设置。"
      : "选择候选窗和悬浮工具栏使用的主题；明暗预览仅影响当前卡片，不修改设置。";

  return <PageIntro>{text}</PageIntro>;
}
