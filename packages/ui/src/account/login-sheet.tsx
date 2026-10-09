import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { privacyUrl } from "../settings/app-resources";
import { AccountInputField } from "./account-input-field";
import { AccountStatusMessages } from "./account-status-messages";
import { useAccountAction } from "./account-operation";
import * as account from "./account-style";
import type {
  AccountChallenge,
  AccountClient,
  AccountProviders,
  AccountUser,
} from "./account-page";

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
  const titleId = useId();
  const subtitleId = useId();

  useLayoutEffect(() => {
    const active = document.activeElement;
    const opener = active instanceof HTMLElement ? active : null;
    // 标了 `data-sheet-autofocus` 的输入框优先获得焦点；否则由第一个可用按钮获得。
    const target =
      dialog.current?.querySelector<HTMLElement>("[data-sheet-autofocus]:not(:disabled)") ??
      dialog.current?.querySelector<HTMLElement>("button:not(:disabled)");
    target?.focus();
    return () => {
      if (
        opener?.isConnected &&
        (!document.activeElement || document.activeElement === document.body)
      )
        opener.focus();
    };
  }, []);

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
        onKeyDown={(event) => {
          if (event.key !== "Escape") return;
          event.stopPropagation();
          close();
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
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.6"
              strokeLinecap="round"
              aria-hidden="true"
            >
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

type Channel = "email" | "phone";

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
  const [channel, setChannel] = useState<Channel | null>(null);
  const [target, setTarget] = useState("");
  const [code, setCode] = useState("");
  const [challenge, setChallenge] = useState<AccountChallenge | null>(null);
  const [expiresAt, setExpiresAt] = useState(0);
  const [resendAt, setResendAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());
  const [googleWaiting, setGoogleWaiting] = useState(false);
  const googleWaitingRef = useRef(false);
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setError,
    setNotice,
  );

  const cancelGoogle = () => {
    if (!googleWaitingRef.current || !client.googleCancel) return;
    // 同一次等待只取消一次：关面板时 onClose 和卸载清理都会走到这里。
    googleWaitingRef.current = false;
    // 等着的 googleLogin 会报告结果；取消失败只说明已经没有可取消的了。
    void client.googleCancel().catch(() => undefined);
  };

  // 关掉面板或换了客户端时，不留一个还在等浏览器的本机监听。
  useEffect(() => () => cancelGoogle(), [client]);

  useEffect(() => {
    if (!challenge) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [challenge]);

  const chooseChannel = (value: Channel) => {
    if (value === channel) return;
    setChannel(value);
    setTarget("");
    setCode("");
    setChallenge(null);
    setError("");
    setNotice("");
  };

  const requestCode = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!channel) return;
      const normalized = target.trim();
      if (!normalized) throw { code: "account_invalid" };
      const value = await client.requestCode(channel, normalized);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const timestamp = Date.now();
      setNow(timestamp);
      setChallenge(value);
      setExpiresAt(timestamp + value.expiresIn * 1000);
      setResendAt(timestamp + 60_000);
      setCode("");
      setNotice("验证码已发送，请查收。");
    });

  const signIn = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!challenge || !/^\d{6}$/.test(code) || expiresAt <= Date.now())
        throw { code: "account_invalid" };
      const result = await client.login(challenge.challengeId, code);
      if (!mounted.current || generation !== clientGeneration.current) return;
      if (!result.user) throw { code: "account_unavailable" };
      onSignedIn(result.user);
    });

  const signInWithGoogle = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!client.googleLogin) throw { code: "account_unavailable" };
      googleWaitingRef.current = true;
      setGoogleWaiting(true);
      let result: { user?: AccountUser | null };
      try {
        result = await client.googleLogin();
      } finally {
        if (mounted.current && generation === clientGeneration.current) {
          googleWaitingRef.current = false;
          setGoogleWaiting(false);
        }
      }
      if (!mounted.current || generation !== clientGeneration.current) return;
      if (!result.user) throw { code: "account_unavailable" };
      onSignedIn(result.user);
    });

  const refreshProviders = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      const value = await client.providers();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onProvidersChange(value);
    });

  const clearExpired = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.clearExpired();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onClearedExpired();
    });

  const resendSeconds = Math.max(0, Math.ceil((resendAt - now) / 1000));
  const expired = Boolean(challenge) && expiresAt <= now;
  const targetLabel = channel === "email" ? "邮箱地址" : "手机号（含国家区号）";
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
          <>
            <AccountInputField
              label={<span className="sr-only">{targetLabel}</span>}
              ariaLabel={targetLabel}
              className={account.sheetInput}
              type={channel === "email" ? "email" : "tel"}
              autoComplete={channel === "email" ? "email" : "tel"}
              placeholder={channel === "email" ? "name@example.com" : targetLabel}
              maxLength={320}
              value={target}
              disabled={busy}
              autoFocus
              onChange={(value) => {
                setTarget(value);
                setChallenge(null);
                setCode("");
              }}
            />
            <button
              type="button"
              className={account.sheetPrimary}
              disabled={busy || !target.trim() || resendSeconds > 0}
              onClick={requestCode}
            >
              {resendSeconds > 0
                ? `${resendSeconds} 秒后可重新发送`
                : challenge
                  ? "重新发送验证码"
                  : "发送验证码"}
            </button>
            {challenge && (
              <>
                <AccountInputField
                  label={<span className="sr-only">6 位验证码</span>}
                  ariaLabel="6 位验证码"
                  className={account.sheetInput}
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  placeholder="6 位验证码"
                  maxLength={6}
                  value={code}
                  disabled={busy}
                  onChange={(value) => setCode(value.replace(/\D/g, "").slice(0, 6))}
                />
                <button
                  type="button"
                  className={account.sheetPrimary}
                  disabled={busy || expired || !/^\d{6}$/.test(code)}
                  onClick={signIn}
                >
                  {expired ? "验证码已过期，请重新获取" : busy ? "正在登录…" : "登录"}
                </button>
              </>
            )}
            <p className={`${account.sheetHint} text-center`}>
              验证码只用于本次登录，请勿向他人透露。
            </p>
          </>
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
