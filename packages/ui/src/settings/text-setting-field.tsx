import type { InputHTMLAttributes, ReactNode } from "react";
import { SettingField } from "./setting-field";

export interface TextSettingFieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "value" | "onChange" | "aria-label"
> {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  value: string;
  onChange: (value: string) => void;
}

/** Shared labeled text input. */
export function TextSettingField({
  label,
  inputLabel,
  description,
  value,
  onChange,
  ...inputProps
}: TextSettingFieldProps) {
  return (
    <SettingField label={label} description={description}>
      <input
        {...inputProps}
        aria-label={inputLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </SettingField>
  );
}
