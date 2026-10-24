import { isComposingKeyEvent } from "../core/ime-key-event";
import { CloseGlyph } from "../core/close-glyph";
import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { privacyUrl } from "../settings/app-resources";
import { AccountVerificationForm } from "./account-verification-form";
import { AccountStatusMessages } from "./account-status-messages";
import { useAccountAction } from "./account-operation";
import { useAccountRecoveryActions } from "./use-account-recovery-actions";
import { useAccountGoogleLogin } from "./use-account-google-login";
import { useAccountVerification, type AccountChannel } from "./use-account-verification";
import * as account from "./account-style";
import type { AccountClient, AccountProviders, AccountUser } from "./account-model";

/**
 * HarmonyOS 手机的底部面板：页面上覆盖一层 .35 的遮罩，再从底边升起一张卡片，带拖动条、22px 标题、可选副标题和 30px 关闭按钮。遮罩、关闭按钮和 Escape 都能关闭它，除非 `closeDisabled` 在其操作进行期间让它保持打开。
 *
 * 它就地渲染、固定在视口上，因此继承打开它的页面的平台和季节 token。打开时焦点移入面板，关闭时回到打开它的控件。
 */
export function BottomSheet({
  title,
  subtitle,
  closeDisabled,
  onClose,
  children,
}: {
  title: string;
  subtitle?: string;
  closeDisabled?: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  const focusedControl = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const subtitleId = useId();

  useLayoutEffect(() => {
    const active = document.activeElement;
    const opener = active instanceof HTMLElement ? active : null;
    // 标记的输入框优先获得焦点，其次是可用按钮；全部禁用时由容器接住键盘导航。
    const target =
      dialog.current?.querySelector<HTMLElement>("[data-sheet-autofocus]:not(:disabled)") ??
      dialog.current?.querySelector<HTMLElement>("button:not(:disabled)");
    (target ?? dialog.current)?.focus();
    return () => {
      if (
        opener?.isConnected &&
        (!document.activeElement ||
          document.activeElement === document.body ||
          dialog.current?.contains(document.activeElement))
      )
        opener.focus();
    };
  }, []);

  useLayoutEffect(() => {
    const panel = dialog.current;
    const previous = focusedControl.current;
    // 浏览器禁用或移除当前控件时可能把焦点交给 body，先由面板接住下一次 Tab。
    if (
      panel &&
      previous &&
      panel.ownerDocument.activeElement === panel.ownerDocument.body &&
      (!previous.isConnected || previous.matches(":disabled"))
    )
      panel.focus();
  });

  const close = () => {
    if (!closeDisabled) onClose();
  };

  return (
    <div className="contents">
      <div className={account.sheetScrim} aria-hidden="true" onClick={close} />
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={subtitle ? subtitleId : undefined}
        className={account.sheet}
        tabIndex={-1}
        onFocusCapture={(event) => {
          focusedControl.current = event.target;
        }}
        onBlurCapture={(event) => {
          if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget))
            focusedControl.current = null;
        }}
        onKeyDown={(event) => {
          if (isComposingKeyEvent(event)) return;
          if (event.key === "Escape") {
            event.stopPropagation();
            close();
            return;
          }
          if (event.key !== "Tab") return;
          // 表单内容和禁用状态会变化，按当前可参与 Tab 导航的控件循环焦点。
          const focusable = Array.from(
            event.currentTarget.querySelectorAll<HTMLElement>(
              'a[href], button, input:not([type="hidden"]), select, textarea, [tabindex]',
            ),
          ).filter(
            (element) =>
              element.tabIndex >= 0 &&
              !element.matches(":disabled") &&
              !element.closest("[hidden], [inert]"),
          );
          const first = focusable[0];
          const last = focusable.at(-1);
          const active = event.currentTarget.ownerDocument.activeElement;
          if (!first || !last) {
            event.preventDefault();
            event.currentTarget.focus();
          } else if (event.shiftKey && (active === first || active === event.currentTarget)) {
            event.preventDefault();
            last.focus();
          } else if (!event.shiftKey && (active === last || active === event.currentTarget)) {
            event.preventDefault();
            first.focus();
          }
        }}
      >
        <span className={account.sheetGrabber} aria-hidden="true" />
        <div className={account.sheetHeader}>
          <div className={account.sheetHeading}>
            <h2 id={titleId} className={account.sheetTitle}>
              {title}
            </h2>
            {subtitle && (
              <p id={subtitleId} className={account.sheetSubtitle}>
                {subtitle}
              </p>
            )}
          </div>
          <button
            type="button"
            className={account.sheetClose}
            aria-label="关闭"
            disabled={closeDisabled}
            onClick={close}
          >
            <CloseGlyph size={14} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

/**
 * HarmonyOS 手机上的「登录水杉」：把页面原先内嵌绘制的邮箱和手机验证码登录放进设计里的底部面板，再加上宿主提供时的 Google 登录。HarmonyOS 没有 Apple 登录；Google 登录由宿主用系统浏览器加本机回环完成（`googleLogin`），面板只显示等待和取消。页面从未登录的个人资料卡片打开它，其他页面要求登录时也直接打开。
 *
 * 验证码被接受后 `onSignedIn` 收到用户信息；页面随后关闭面板并加载个人资料。修复操作（刷新登录方式、清除失效登录状态）以低调的链接留在底部。
 */
export function LoginSheet({
  client,
  providers,
  onProvidersChange,
  onSignedIn,
  onClearedExpired,
  onClose,
  onOpenUrl,
}: {
  client: AccountClient;
  providers: AccountProviders;
  onProvidersChange: (providers: AccountProviders) => void;
  onSignedIn: (user: AccountUser) => void;
  onClearedExpired: () => void;
  onClose: () => void;
  onOpenUrl?: (url: string) => void;
}) {
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setError,
    setNotice,
  );
  const { refreshProviders, clearExpired } = useAccountRecoveryActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onProvidersChange,
    onClearedExpired,
  });
  const verification = useAccountVerification({ client, mounted, clientGeneration, setNotice });
  const { channel, selectChannel, sendCode, signIn: runVerificationSignIn } = verification;
  const {
    waiting: googleWaiting,
    cancel: cancelGoogle,
    signIn: runGoogleLogin,
  } = useAccountGoogleLogin({ client, mounted, clientGeneration, cancelOnce: true });

  // 关掉面板或换了客户端时，不留一个还在等浏览器的本机监听。
  useEffect(() => () => cancelGoogle(), [client]);

  const chooseChannel = (value: AccountChannel) => {
    if (value === channel) return;
    selectChannel(value);
    setError("");
    setNotice("");
  };

  const requestCode = () => void perform(sendCode);

  const signIn = () =>
    void perform(async () => {
      const result = await runVerificationSignIn();
      if (result) onSignedIn(result.user);
    });

  const signInWithGoogle = () =>
    void perform(async () => {
      const result = await runGoogleLogin();
      if (result) onSignedIn(result.user);
    });

  // 后端可能为没有对应客户端的宿主也开着 Google，所以还要看宿主有没有 googleLogin。
  const googleAvailable = providers.google === true && Boolean(client.googleLogin);

  return (
    <BottomSheet
      title="登录水杉"
      subtitle="在手机、平板和电脑之间同步词库、皮肤和云剪贴板"
      // 等浏览器时可以关面板，关掉就等于取消；其他操作进行中仍不能关。
      closeDisabled={busy && !googleWaiting}
      onClose={() => {
        cancelGoogle();
        onClose();
      }}
    >
      <AccountStatusMessages error={error} notice={notice} />
      <div className={account.sheetBody}>
        {googleAvailable && (
          <button
            type="button"
            className={account.sheetChoice}
            disabled={busy}
            onClick={signInWithGoogle}
          >
            {googleWaiting ? "正在等待浏览器完成 Google 登录…" : "使用 Google 登录"}
          </button>
        )}
        {googleWaiting && client.googleCancel && (
          <button type="button" className={account.link} onClick={cancelGoogle}>
            取消 Google 登录
          </button>
        )}
        {providers.email && (
          <button
            type="button"
            className={account.sheetChoice}
            aria-pressed={channel === "email"}
            disabled={busy}
            onClick={() => chooseChannel("email")}
          >
            使用邮箱登录
          </button>
        )}
        {providers.phone && (
          <button
            type="button"
            className={account.sheetChoice}
            aria-pressed={channel === "phone"}
            disabled={busy}
            onClick={() => chooseChannel("phone")}
          >
            使用手机号登录
          </button>
        )}
        {!providers.email && !providers.phone && !googleAvailable && (
          <p className={`${account.sheetHint} text-center`}>当前没有可用的登录方式，请稍后重试。</p>
        )}
        {channel && (
          <AccountVerificationForm
            variant="sheet"
            verification={verification}
            busy={busy}
            onRequestCode={requestCode}
            onSignIn={signIn}
          />
        )}
      </div>
      <p className={account.sheetFooter}>
        登录即表示同意《用户协议》和
        {onOpenUrl ? (
          <button
            type="button"
            className={account.sheetFooterLink}
            onClick={() => onOpenUrl(privacyUrl)}
          >
            《隐私政策》
          </button>
        ) : (
          "《隐私政策》"
        )}
      </p>
      <div className={account.sheetQuiet}>
        <button type="button" className={account.link} disabled={busy} onClick={refreshProviders}>
          刷新登录方式
        </button>
        <button type="button" className={account.link} disabled={busy} onClick={clearExpired}>
          清除失效登录状态
        </button>
      </div>
    </BottomSheet>
  );
}
