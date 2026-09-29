import * as settings from "./settings-style";

export interface ShortcutsIntroSectionProps {
  mobile: boolean;
}

/** Scope explanation shown above the physical shortcut controls. */
export function ShortcutsIntroSection({ mobile }: ShortcutsIntroSectionProps) {
  return (
    <div className={`section ${settings.shortcutIntro}`}>
      {mobile
        ? "输入法快捷键仅在对应输入状态或候选栏显示时生效。翻页方式可在“输入”中启用或关闭。"
        : "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式可在“输入”中启用或关闭。"}
    </div>
  );
}
