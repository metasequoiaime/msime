import type { ReactNode, SelectHTMLAttributes } from "react";
import { SettingField } from "./setting-field";

export interface SelectSettingFieldProps extends Omit<
  SelectHTMLAttributes<HTMLSelectElement>,
  "value" | "onChange" | "aria-label" | "children"
> {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  fieldClassName?: string;
  value: string;
  onChange: (value: string) => void;
  children?: ReactNode;
}

/** Shared labeled select control for legacy settings fields. */
export function SelectSettingField({
  label,
  inputLabel,
  description,
  fieldClassName,
  value,
  onChange,
  children,
  ...selectProps
}: SelectSettingFieldProps) {
  return (
    <SettingField label={label} description={description} className={fieldClassName}>
      <select
        {...selectProps}
        aria-label={inputLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {children}
      </select>
    </SettingField>
  );
}
