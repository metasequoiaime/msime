import * as settings from "./settings-style";

export type FloatingToolbarScale = 75 | 100 | 125 | 150;
export type FloatingToolbarFontSize = 16 | 18 | 20 | 22 | 24 | 26 | 28;

export interface FloatingToolbarAppearanceSectionProps {
  scale: FloatingToolbarScale;
  fontSize: FloatingToolbarFontSize;
  onScaleChange: (value: FloatingToolbarScale) => void;
  onFontSizeChange: (value: FloatingToolbarFontSize) => void;
}

const scales: FloatingToolbarScale[] = [75, 100, 125, 150];
const fontSizes: FloatingToolbarFontSize[] = [16, 18, 20, 22, 24, 26, 28];

/** Scale and icon size controls for the floating toolbar settings page. */
export function FloatingToolbarAppearanceSection({
  scale,
  fontSize,
  onScaleChange,
  onFontSizeChange,
}: FloatingToolbarAppearanceSectionProps) {
  return (
    <div className={`section ${settings.toolbarAppearanceHeader}`}>
      <label className="section-header">
        <span className="section-title">
          工具栏缩放<small>相对系统 DPI 的额外缩放，不改变系统显示缩放</small>
        </span>
        <select
          aria-label="工具栏缩放"
          value={scale}
          onChange={(event) => onScaleChange(Number(event.target.value) as FloatingToolbarScale)}
        >
          {scales.map((value) => (
            <option key={value} value={value}>
              {value}%
            </option>
          ))}
        </select>
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">
          图标尺寸<small>图标基准大小（像素），再乘以上方缩放</small>
        </span>
        <select
          aria-label="图标尺寸"
          value={fontSize}
          onChange={(event) =>
            onFontSizeChange(Number(event.target.value) as FloatingToolbarFontSize)
          }
        >
          {fontSizes.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
