import type { ReactNode } from "react";

export interface SettingCheckProps {
  label: ReactNode;
  checked: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  onChange: (checked: boolean) => void;
}

/** Shared compact checkbox row for option lists inside a settings section. */
export function SettingCheck({ label, checked, disabled, ariaLabel, onChange }: SettingCheckProps) {
  return (
    <label className="check-option">
      <input
        type="checkbox"
        aria-label={ariaLabel}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}
