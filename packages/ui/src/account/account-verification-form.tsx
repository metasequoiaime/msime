import { ActionButton } from "../core/action-button";
import { isComposingKeyEvent } from "../core/ime-key-event";
import { AccountInputField } from "./account-input-field";
import { AccountVerificationCodeField } from "./account-verification-code-field";
import type { useAccountVerification } from "./use-account-verification";
import * as account from "./account-style";

type VerificationFields = Pick<
  ReturnType<typeof useAccountVerification>,
  | "channel"
  | "target"
  | "updateTarget"
  | "code"
  | "setCode"
  | "challenge"
  | "resendSeconds"
  | "expired"
>;

/** Shared verification controls with the account page and login sheet presentations. */
export function AccountVerificationForm({
  variant,
  verification,
  busy,
  onRequestCode,
  onSignIn,
}: {
  variant: "page" | "sheet";
  verification: VerificationFields;
  busy: boolean;
  onRequestCode: () => void;
  onSignIn: () => void;
}) {
  const { channel, target, updateTarget, code, setCode, challenge, resendSeconds, expired } =
    verification;
  if (!channel) return null;

  const sheet = variant === "sheet";
  const targetLabel = channel === "email" ? "邮箱地址" : "手机号（含国家区号）";
  const requestLabel =
    resendSeconds > 0
      ? `${resendSeconds} 秒后可重新发送`
      : sheet
        ? challenge
          ? "重新发送验证码"
          : "发送验证码"
        : "获取验证码";
  const codeControls = (
    <>
      <AccountVerificationCodeField
        className={sheet ? account.sheetInput : undefined}
        visuallyHiddenLabel={sheet}
        placeholder={sheet ? "6 位验证码" : undefined}
        value={code}
        disabled={busy}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || isComposingKeyEvent(event)) return;
          if (busy || expired || !/^\d{6}$/.test(code)) return;
          event.preventDefault();
          onSignIn();
        }}
        onChange={setCode}
      />
      <ActionButton
        action={onSignIn}
        className={sheet ? account.sheetPrimary : account.submit}
        disabled={busy || expired || !/^\d{6}$/.test(code)}
        label={expired ? "验证码已过期，请重新获取" : busy ? "正在登录…" : "登录"}
      />
    </>
  );

  return (
    <>
      <AccountInputField
        label={sheet ? <span className="sr-only">{targetLabel}</span> : targetLabel}
        ariaLabel={targetLabel}
        className={sheet ? account.sheetInput : undefined}
        type={channel === "email" ? "email" : "tel"}
        autoComplete={channel === "email" ? "email" : "tel"}
        placeholder={sheet ? (channel === "email" ? "name@example.com" : targetLabel) : undefined}
        maxLength={320}
        value={target}
        disabled={busy}
        autoFocus={sheet}
        onChange={updateTarget}
      />
      <ActionButton
        action={onRequestCode}
        className={sheet ? account.sheetPrimary : challenge ? account.provider : account.submit}
        disabled={busy || !target.trim() || resendSeconds > 0}
        label={requestLabel}
      />
      {challenge && (sheet ? codeControls : <div className={account.code}>{codeControls}</div>)}
      <p
        className={sheet ? `${account.sheetHint} text-center` : `${account.muted} m-0 text-center`}
      >
        验证码只用于本次登录，请勿向他人透露。
      </p>
    </>
  );
}
