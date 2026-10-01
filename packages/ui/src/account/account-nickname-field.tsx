import * as account from "./account-style";

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
    <label className={account.field}>
      社区昵称
      <input
        className={account.input}
        aria-label={ariaLabel}
        maxLength={64}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
