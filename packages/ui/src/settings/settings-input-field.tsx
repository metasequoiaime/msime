import type { InputHTMLAttributes, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsInputFieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "value" | "onChange" | "aria-label" | "className"
> {
  label: ReactNode;
  ariaLabel: string;
  value: string;
  className?: string;
  onChange: (value: string) => void;
}

/** Shared stacked text input used by settings manager forms. */
export function SettingsInputField({
  label,
  ariaLabel,
  value,
  className = settings.fieldInput,
  onChange,
  ...inputProps
}: SettingsInputFieldProps) {
  return (
    <label className={settings.field}>
      {label}
      <input
        {...inputProps}
        className={className}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
