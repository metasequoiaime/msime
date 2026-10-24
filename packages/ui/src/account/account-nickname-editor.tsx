import { AccountInputField } from "./account-input-field";
import { AccountNicknameField } from "./account-nickname-field";
import * as account from "./account-style";

export const accountNicknameHelpText = "昵称会显示在社区作品中，已发布的作品也会同步更新。";
const emptyNameText = "取一个喜欢的名字，让大家记住你。";

/** 资料页共用的昵称输入与提示，业务规则和长度上限来自共享 Rust。 */
export function AccountNicknameEditor({
  variant,
  value,
  normalizedName,
  validName,
  disabled,
  maximumChars,
  maximumUtf16Units,
  checking,
  unavailable,
  onChange,
}: {
  variant: "mobile" | "sheet" | "desktop";
  value: string;
  normalizedName: string;
  validName: boolean;
  disabled: boolean;
  maximumChars?: number;
  maximumUtf16Units?: number;
  checking: boolean;
  unavailable: boolean;
  onChange: (value: string) => void;
}) {
  const count = [...normalizedName].length;
  const invalidNameText =
    maximumChars === undefined
      ? "请勿使用换行或控制字符。"
      : `昵称最多 ${maximumChars} 个字符，请勿使用换行或控制字符。`;
  const counter = maximumChars === undefined ? String(count) : `${count}/${maximumChars}`;
  const guidance = unavailable
    ? "昵称校验暂不可用，请稍后重试。"
    : checking
      ? "正在检查昵称…"
      : normalizedName
        ? validName
          ? accountNicknameHelpText
          : invalidNameText
        : emptyNameText;

  const hasError = unavailable || (!checking && !validName && !!normalizedName);

  if (variant === "sheet")
    return (
      <>
        <AccountInputField
          label={<span className="sr-only">社区昵称</span>}
          ariaLabel="编辑社区昵称"
          className={account.sheetInput}
          maxLength={maximumUtf16Units}
          value={value}
          disabled={disabled}
          data-sheet-autofocus=""
          onChange={onChange}
        />
        <p className={hasError ? account.sheetHintError : account.sheetHint}>
          {!checking && normalizedName && validName ? counter : guidance}
        </p>
      </>
    );

  return (
    <>
      <AccountNicknameField
        value={value}
        disabled={disabled}
        maxLength={maximumUtf16Units}
        ariaLabel="编辑社区昵称"
        onChange={onChange}
      />
      {variant === "mobile" ? (
        <>
          <p className={`account-muted${hasError ? " error" : ""}`}>{guidance}</p>
          <p className={account.muted}>{counter}</p>
        </>
      ) : (
        <p className={hasError ? `${account.muted} error` : account.muted}>{guidance}</p>
      )}
    </>
  );
}
