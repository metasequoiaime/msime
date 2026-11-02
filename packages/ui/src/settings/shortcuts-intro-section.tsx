import { SettingsGroupNote } from "./settings-group-note";

export interface ShortcutsIntroSectionProps {
  mobile: boolean;
}

/** 快捷键页的总说明：快捷键在什么状态下生效。放在「输入模式切换」组里，收进组名旁的「?」，不常驻页首。 */
export function ShortcutsIntroSection({ mobile }: ShortcutsIntroSectionProps) {
  return (
    <SettingsGroupNote>
      {mobile
        ? "输入法快捷键仅在对应输入状态或候选栏显示时生效。翻页方式和以词定字在「输入 › 选词与翻页」中设置。"
        : "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式和以词定字在「输入 › 选词与翻页」中设置。"}
    </SettingsGroupNote>
  );
}
