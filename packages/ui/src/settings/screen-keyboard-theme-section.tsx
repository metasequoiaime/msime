import { SurfaceThemeRow } from "./surface-theme-row";
import type { SurfaceTheme } from "./surface-theme-select";

export interface ScreenKeyboardThemeSectionProps {
  mobile: boolean;
  value: SurfaceTheme;
  onChange: (value: SurfaceTheme) => void;
}

/** The screen keyboard's own light/dark override, one of the per-surface rows in the 主题 page's 高级 group. */
export function ScreenKeyboardThemeSection({
  mobile,
  value,
  onChange,
}: ScreenKeyboardThemeSectionProps) {
  return (
    <SurfaceThemeRow
      title="屏幕键盘主题"
      description={
        mobile ? "覆盖颜色模式；当前屏幕键盘支持此设置" : "覆盖颜色模式；桌面屏幕键盘支持此设置"
      }
      value={value}
      onChange={onChange}
    />
  );
}
