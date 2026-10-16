import type { InputHTMLAttributes, ReactNode } from "react";
import { LabeledInputField } from "../core/labeled-input-field";
import * as style from "./community-style";

export interface CommunityInputFieldProps {
  label: ReactNode;
  ariaLabel: string;
  value: string;
  type?: InputHTMLAttributes<HTMLInputElement>["type"];
  maxLength?: number;
  disabled?: boolean;
  placeholder?: string;
  onChange: (value: string) => void;
}

/** 社区表单共用的带样式文本输入框。 */
export function CommunityInputField({
  label,
  ariaLabel,
  value,
  type,
  maxLength,
  disabled,
  placeholder,
  onChange,
}: CommunityInputFieldProps) {
  return (
    <LabeledInputField
      label={label}
      labelClassName={style.field}
      className={style.fieldControl}
      ariaLabel={ariaLabel}
      type={type}
      maxLength={maxLength}
      value={value}
      disabled={disabled}
      placeholder={placeholder}
      onChange={onChange}
    />
  );
}
