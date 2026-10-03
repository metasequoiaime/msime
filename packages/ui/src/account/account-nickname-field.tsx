import { AccountInputField } from "./account-input-field";

export interface AccountNicknameFieldProps {
  value: string;
  disabled?: boolean;
  ariaLabel?: string;
  onChange: (value: string) => void;
}

/** Shared community nickname input used by the account profile surfaces. */
export function AccountNicknameField({
  value,
  disabled,
  ariaLabel = "社区昵称",
  onChange,
}: AccountNicknameFieldProps) {
  return (
    <AccountInputField
      label="社区昵称"
      ariaLabel={ariaLabel}
      maxLength={64}
      value={value}
      disabled={disabled}
      onChange={onChange}
    />
  );
}
