import { SettingsGroupNote } from "./settings-group-note";

/** Explains why the candidate palette controls are inactive while the keyboard skin supplies colors. */
export function CandidatePaletteFallbackNotice() {
  return (
    <SettingsGroupNote>
      候选栏正在使用键盘皮肤的颜色，下面的候选颜色要打开「候选栏使用主题配色」后才生效。
    </SettingsGroupNote>
  );
}
