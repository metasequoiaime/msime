import type { ReactNode } from "react";
import { ControlledInput } from "../core/controlled-input";
import { useCredentialFocus } from "../core/use-credential-focus";
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
  const { inputRef } = useCredentialFocus(true);
  return (
    <SettingField label={label} description={description}>
      <ControlledInput
        ref={inputRef}
        aria-label={inputLabel}
        type="password"
        autoComplete="off"
        value={value}
        disabled={disabled}
        onChange={onChange}
      />
    </SettingField>
  );
}
