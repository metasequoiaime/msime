import type { InputHTMLAttributes, ReactNode } from "react";
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

/** Shared styled text input field used by community forms. */
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
    <label className={style.field}>
      {label}
      <input
        className={style.fieldControl}
        aria-label={ariaLabel}
        type={type}
        maxLength={maxLength}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
