import { useId, useRef, useState } from "react";
import { privacyUrl, termsUrl } from "../settings/app-resources";
import * as account from "./account-style";

/** 没勾选就去登录时的提示，同时是提示段落的文字。 */
export const LOGIN_CONSENT_PROMPT = "请先阅读并勾选同意《用户协议》和《隐私政策》";

/**
 * 登录前的明示同意（《个人信息保护法》要求的「明示同意」，AppGallery 审核按它驳回过「登录即代表同意」的写法）：勾选框默认不勾，用户自己勾上之前，发送验证码、验证码登录、华为账号、Google 和 Apple 登录都不发出任何请求。
 *
 * 没勾就点了这些操作时，`confirm` 返回 false，显示提示并把焦点移到勾选框上；勾上后提示消失。勾选只活在当前这次登录界面里，不落盘：每次打开登录都要重新勾。
 */
export function useLoginConsent() {
  const [agreed, setAgreedState] = useState(false);
  const [prompting, setPrompting] = useState(false);
  const checkbox = useRef<HTMLInputElement>(null);
  const setAgreed = (value: boolean) => {
    setAgreedState(value);
    if (value) setPrompting(false);
  };
  const confirm = () => {
    if (agreed) return true;
    setPrompting(true);
    checkbox.current?.focus();
    return false;
  };
  const reset = () => {
    setAgreedState(false);
    setPrompting(false);
  };
  return { agreed, setAgreed, prompting, confirm, reset, checkbox };
}

export type LoginConsentState = ReturnType<typeof useLoginConsent>;

/**
 * 「我已阅读并同意《用户协议》和《隐私政策》」一行，两个文档名各自打开 msime.app 上对应的网页。以前没有用户协议页面时这里只列隐私政策，因为列一个打不开的文档名正是审核驳回的「用户协议点击无响应」。
 *
 * 宿主不能打开网页（没有 `onOpenUrl`）时文档名是普通文字。
 */
export function LoginConsent({
  consent,
  onOpenUrl,
}: {
  consent: LoginConsentState;
  onOpenUrl?: (url: string) => void;
}) {
  const promptId = useId();
  return (
    <div className={account.consent}>
      <p className={account.consentLine}>
        <label className={account.consentLabel}>
          <input
            ref={consent.checkbox}
            type="checkbox"
            className={account.consentBox}
            aria-label="我已阅读并同意《用户协议》和《隐私政策》"
            aria-describedby={consent.prompting ? promptId : undefined}
            checked={consent.agreed}
            onChange={(event) => consent.setAgreed(event.target.checked)}
          />
          我已阅读并同意
        </label>
        <ConsentDocument title="《用户协议》" url={termsUrl} onOpenUrl={onOpenUrl} />和
        <ConsentDocument title="《隐私政策》" url={privacyUrl} onOpenUrl={onOpenUrl} />
      </p>
      {consent.prompting && (
        <p id={promptId} role="alert" className={account.consentPrompt}>
          {LOGIN_CONSENT_PROMPT}
        </p>
      )}
    </div>
  );
}

function ConsentDocument({
  title,
  url,
  onOpenUrl,
}: {
  title: string;
  url: string;
  onOpenUrl?: (url: string) => void;
}) {
  if (!onOpenUrl) return <>{title}</>;
  return (
    <button type="button" className={account.consentLink} onClick={() => onOpenUrl(url)}>
      {title}
    </button>
  );
}
