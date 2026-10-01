import { Row } from "../core/platform-controls";
import { SurfaceThemeSelect, type SurfaceTheme } from "./surface-theme-select";

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
    <Row
      title="屏幕键盘主题"
      description={
        mobile ? "覆盖主题模式；当前屏幕键盘支持此设置" : "覆盖主题模式；桌面屏幕键盘支持此设置"
      }
    >
      <SurfaceThemeSelect label="屏幕键盘主题" value={value} onChange={onChange} />
    </Row>
  );
}
