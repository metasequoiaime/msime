import type { ReactNode } from "react";

export interface SettingToggleProps {
  label: ReactNode;
  description: ReactNode;
  checked: boolean;
  ariaLabel?: string;
  disabled?: boolean;
  /** Render only the labeled row when the toggle is embedded in another section. */
  compact?: boolean;
  onChange: (checked: boolean) => void;
}

/** Shared labeled checkbox row for settings that toggle one preference. */
export function SettingToggle({
  label,
  description,
  checked,
  ariaLabel,
  disabled,
  compact = false,
  onChange,
}: SettingToggleProps) {
  const row = (
    <label className="section-header">
      <span className="section-title">
        {label}
        <small>{description}</small>
      </span>
      <input
        className="toggle"
        type="checkbox"
        aria-label={ariaLabel}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
  return compact ? row : <div className="section">{row}</div>;
}
