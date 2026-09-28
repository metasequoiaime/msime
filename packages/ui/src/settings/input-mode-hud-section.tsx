export interface InputModeHudSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
  /** Render the compact row used inside the shortcut group. */
  shortcut?: boolean;
}

/** Input-mode badge switch shared by the appearance and shortcut settings pages. */
export function InputModeHudSection({
  value,
  onChange,
  shortcut = false,
}: InputModeHudSectionProps) {
  const label = shortcut ? "切换中英文时显示提示" : "中英文切换提示";
  const description = shortcut
    ? "切换后在光标下方短暂显示「中」或「英」。"
    : "切换输入模式后，在光标附近短暂显示“中”或“英”，不会抢占焦点。";
  const control = (
    <label className="section-header">
      <span className="section-title">
        {label}
        <small>{description}</small>
      </span>
      <input
        aria-label={label}
        className="toggle"
        type="checkbox"
        checked={value ?? true}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
  return shortcut ? control : <div className="section">{control}</div>;
}
