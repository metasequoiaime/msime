import type { ReactNode } from "react";
import { SecretInput } from "../core/secret-input";
import { SettingField } from "./setting-field";

export interface SecretSettingFieldProps {
  label: ReactNode;
  inputLabel?: string;
  description?: ReactNode;
  /** 状态或错误，常驻标题下方。 */
  detail?: ReactNode;
  value: string;
  placeholder?: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

/** Shared labeled secret input. */
export function SecretSettingField({
  label,
  inputLabel,
  description,
  detail,
  value,
  placeholder,
  disabled,
  onChange,
}: SecretSettingFieldProps) {
  return (
    <SettingField label={label} description={description} detail={detail}>
      <SecretInput
        label={inputLabel ?? String(label)}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={onChange}
      />
    </SettingField>
  );
}
