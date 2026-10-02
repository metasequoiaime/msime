import type { ReactNode } from "react";
import { EndpointInput } from "./endpoint-input";
import { SettingField } from "./setting-field";

export interface EndpointSettingFieldProps {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  value: string;
  disabled?: boolean;
  placeholder?: string;
  onChange: (value: string) => void;
}

/** Shared labeled endpoint field for legacy provider settings. */
export function EndpointSettingField({
  label,
  inputLabel,
  description,
  value,
  disabled,
  placeholder,
  onChange,
}: EndpointSettingFieldProps) {
  return (
    <SettingField label={label} description={description}>
      <EndpointInput
        label={inputLabel}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={onChange}
      />
    </SettingField>
  );
}
