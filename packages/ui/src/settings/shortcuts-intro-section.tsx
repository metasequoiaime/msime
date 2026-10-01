import { PageIntro } from "../core/platform-controls";

export interface ShortcutsIntroSectionProps {
  mobile: boolean;
}

/** 快捷键页的页首说明：快捷键在什么状态下生效。 */
export function ShortcutsIntroSection({ mobile }: ShortcutsIntroSectionProps) {
  return (
    <PageIntro>
      {mobile
        ? "输入法快捷键仅在对应输入状态或候选栏显示时生效。翻页方式可在“候选栏”中启用或关闭。"
        : "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式可在“候选窗口”中启用或关闭。"}
    </PageIntro>
  );
}
