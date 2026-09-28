export interface TraditionalChineseOutputSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Simplified-to-traditional output switch shared by input settings hosts. */
export function TraditionalChineseOutputSection({
  value,
  onChange,
}: TraditionalChineseOutputSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          简繁输入<small>将提交的简体中文转换为繁体中文</small>
        </span>
        <input
          aria-label="简繁输入"
          className="toggle"
          type="checkbox"
          checked={value ?? false}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
