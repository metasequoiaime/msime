import { AccountInputField } from "./account-input-field";

export interface AccountNicknameFieldProps {
  value: string;
  disabled?: boolean;
  maxLength?: number;
  ariaLabel?: string;
  onChange: (value: string) => void;
}

/** 资料页共用的昵称输入框，长度上限由共享校验结果传入。 */
export function AccountNicknameField({
  value,
  disabled,
  maxLength,
  ariaLabel = "社区昵称",
  onChange,
}: AccountNicknameFieldProps) {
  return (
    <AccountInputField
      label="社区昵称"
      ariaLabel={ariaLabel}
      maxLength={maxLength}
      value={value}
      disabled={disabled}
      onChange={onChange}
    />
  );
}
