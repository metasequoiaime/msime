import { SettingToggle } from "./setting-toggle";

export interface CandidatePaletteSectionProps {
  value: boolean;
  busy: boolean;
  onChange: (value: boolean) => void;
}

/** Controls whether the mobile candidate strip uses the theme's candidate palette. */
export function CandidatePaletteSection({ value, busy, onChange }: CandidatePaletteSectionProps) {
  return (
    <SettingToggle
      label="候选栏使用主题配色"
      description="关闭时候选栏和按键一起使用键盘皮肤的颜色；打开后使用本页的主题和候选颜色。"
      ariaLabel="候选栏使用主题配色"
      checked={value}
      disabled={busy}
      onChange={onChange}
    />
  );
}
