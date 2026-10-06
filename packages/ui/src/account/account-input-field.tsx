import type { InputHTMLAttributes, ReactNode } from "react";
import * as account from "./account-style";

export interface AccountInputFieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "aria-label" | "value" | "onChange" | "className"
> {
  label: ReactNode;
  ariaLabel?: string;
  value: string;
  className?: string;
  onChange: (value: string) => void;
}

/** Shared labelled input field used by account sign-in and profile forms. */
export function AccountInputField({
  label,
  ariaLabel,
  value,
  className = account.input,
  onChange,
  ...inputProps
}: AccountInputFieldProps) {
  return (
    <label className={account.field}>
      {label}
      <input
        {...inputProps}
        className={className}
        aria-label={ariaLabel ?? String(label)}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
