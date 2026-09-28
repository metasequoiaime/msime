import type { SurfaceTheme } from "./theme-settings-section";

export interface ScreenKeyboardThemeSectionProps {
  mobile: boolean;
  value: SurfaceTheme;
  onChange: (value: SurfaceTheme) => void;
}

/** Theme override used by the shared desktop and mobile screen keyboards. */
export function ScreenKeyboardThemeSection({
  mobile,
  value,
  onChange,
}: ScreenKeyboardThemeSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          屏幕键盘主题
          <small>
            {mobile
              ? "覆盖主题模式；当前屏幕键盘支持此设置"
              : "覆盖主题模式；桌面屏幕键盘支持此设置"}
          </small>
        </span>
        <select
          aria-label="屏幕键盘主题"
          value={value}
          onChange={(event) => onChange(event.target.value as SurfaceTheme)}
        >
          <option value="follow">跟随全局</option>
          <option value="dark">深色</option>
          <option value="light">浅色</option>
        </select>
      </label>
    </div>
  );
}
