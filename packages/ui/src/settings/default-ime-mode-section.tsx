export type DefaultImeMode = "chinese" | "english";

export interface DefaultImeModeSectionProps {
  value?: DefaultImeMode;
  onChange: (value: DefaultImeMode) => void;
}

/** Default language mode selector for new input focus sessions. */
export function DefaultImeModeSection({ value, onChange }: DefaultImeModeSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          默认中英文<small>新焦点会话开始时使用的中文或英文状态</small>
        </span>
        <select
          aria-label="默认中英文"
          value={value ?? "chinese"}
          onChange={(event) => onChange(event.target.value as DefaultImeMode)}
        >
          <option value="chinese">中文</option>
          <option value="english">英文</option>
        </select>
      </label>
    </div>
  );
}
