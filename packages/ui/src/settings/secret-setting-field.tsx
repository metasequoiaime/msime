import type { ReactNode } from "react";
import { SecretInput } from "../core/secret-input";
import { SettingField } from "./setting-field";

export interface SecretSettingFieldProps {
  label: ReactNode;
  inputLabel?: string;
  description?: ReactNode;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** Shared labeled secret input. */
export function SecretSettingField({
  label,
  inputLabel,
  description,
  value,
  disabled,
  onChange,
}: SecretSettingFieldProps) {
  return (
    <SettingField label={label} description={description}>
      <SecretInput
        label={inputLabel ?? String(label)}
        value={value}
        disabled={disabled}
        onChange={onChange}
      />
    </SettingField>
  );
}
