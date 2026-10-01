import type { ReactNode } from "react";
import { SettingField } from "./setting-field";

export interface PasswordSettingFieldProps {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** Shared non-revealable password input for owner-only credential settings. */
export function PasswordSettingField({
  label,
  inputLabel,
  description,
  value,
  disabled,
  onChange,
}: PasswordSettingFieldProps) {
  return (
    <SettingField label={label} description={description}>
      <input
        aria-label={inputLabel}
        type="password"
        autoComplete="off"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </SettingField>
  );
}
