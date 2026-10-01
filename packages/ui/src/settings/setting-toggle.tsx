import type { ReactNode } from "react";
import { SettingField } from "./setting-field";

export interface SettingToggleProps {
  label: ReactNode;
  description?: ReactNode;
  checked: boolean;
  ariaLabel?: string;
  disabled?: boolean;
  rowClassName?: string;
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
  rowClassName,
  compact = false,
  onChange,
}: SettingToggleProps) {
  const row = (
    <SettingField
      label={label}
      description={description}
      className={`section-header${rowClassName ? ` ${rowClassName}` : ""}`}
    >
      <input
        className="toggle"
        type="checkbox"
        aria-label={ariaLabel}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
    </SettingField>
  );
  return compact ? row : <div className="section">{row}</div>;
}
