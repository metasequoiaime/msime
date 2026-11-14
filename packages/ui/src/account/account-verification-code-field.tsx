import type { KeyboardEventHandler } from "react";
import { AccountInputField } from "./account-input-field";

export function AccountVerificationCodeField({
  value,
  onChange,
  disabled,
  visuallyHiddenLabel = false,
  className,
  placeholder,
  onKeyDown,
}: {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  visuallyHiddenLabel?: boolean;
  className?: string;
  placeholder?: string;
  onKeyDown?: KeyboardEventHandler<HTMLInputElement>;
}) {
  return (
    <AccountInputField
      label={visuallyHiddenLabel ? <span className="sr-only">6 位验证码</span> : "6 位验证码"}
      ariaLabel="6 位验证码"
      className={className}
      inputMode="numeric"
      autoComplete="one-time-code"
      placeholder={placeholder}
      maxLength={6}
      value={value}
      disabled={disabled}
      onKeyDown={onKeyDown}
      onChange={(nextValue) => onChange(nextValue.replace(/\D/g, "").slice(0, 6))}
    />
  );
}
