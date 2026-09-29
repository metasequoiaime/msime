import { Row, Select } from "../core/platform-controls";
import type { SurfaceTheme } from "./theme-settings-section";

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
      <Select
        aria-label="屏幕键盘主题"
        value={value}
        onChange={(event) => onChange(event.target.value as SurfaceTheme)}
      >
        <option value="follow">跟随全局</option>
        <option value="dark">深色</option>
        <option value="light">浅色</option>
      </Select>
    </Row>
  );
}
