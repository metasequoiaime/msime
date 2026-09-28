export interface CandidatePaletteSectionProps {
  value: boolean;
  busy: boolean;
  onChange: (value: boolean) => void;
}

/** Controls whether the mobile candidate strip uses the desktop candidate palette. */
export function CandidatePaletteSection({ value, busy, onChange }: CandidatePaletteSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          使用桌面候选皮肤
          <small>
            关闭时候选栏和按键一起使用键盘皮肤的颜色；打开后使用这里的候选皮肤和「外观」里的候选颜色。
          </small>
        </span>
        <input
          aria-label="使用桌面候选皮肤"
          className="toggle"
          type="checkbox"
          disabled={busy}
          checked={value}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
