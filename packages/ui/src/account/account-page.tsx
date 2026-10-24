import type {
  AccountUser,
  AccountProviders,
  AccountProfile,
  AppIconClient,
  AccountClient,
  AccountCommunityDestination,
} from "./account-model";
import { useEffect, useId, useState, type ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import { ActionButton } from "../core/action-button";
import { errorCode } from "../core/error-code";
import { FluentIcon, type FluentIconName } from "../core/fluent-icons";
import type { AppThemeClient } from "../core/host-contracts";
import { GroupList, NavGroup, NavRow, Row } from "../core/platform-controls";
import { useToast } from "../core/toast";
import { useTemporaryFlag } from "../core/use-temporary-flag";
import { privacyUrl } from "../settings/app-resources";
import type { SettingsPageId } from "../settings/settings-page-registry";
import * as doc from "../settings/document-style";
import * as account from "./account-style";
import { AccountAvatar } from "./account-avatar";
import { preferredAccountName } from "./account-labels";
import { accountMessage, isAccountCancellation } from "./account-errors";
import { AccountConfirmation } from "./account-confirmation";
import { HarmonyProfilePage, MobileAccountProfilePage } from "./account-profile-pages";
import { AccountNicknameEditor } from "./account-nickname-editor";
import { AccountVerificationForm } from "./account-verification-form";
import { AccountStatusMessages } from "./account-status-messages";
import { AccountIdentityDetails } from "./account-identity-details";
import { SettingsSyncCard } from "./settings-sync-card";
import { AppIconSettingsCard } from "./app-icon-settings-card";
import { AppThemeRow } from "./app-theme-row";
import { QqLinkCard } from "./qq-link-card";
import { LoginSheet } from "./login-sheet";
import { MobileDownloadPage, otherPlatformCount } from "./mobile-download-page";
import { StatusMessage } from "../core/status-message";
import { copyAccountId as copyAccountIdToClipboard } from "./account-id-copy";
import { useAccountAction } from "./account-operation";
import { useAccountRecoveryActions } from "./use-account-recovery-actions";
import {
  ACCOUNT_DELETED_VOICE_SAMPLES_KEPT,
  runAccountSessionAction,
  useAccountSessionActions,
} from "./use-account-session-actions";
import { useAccountRename } from "./use-account-rename";
import { useAccountGoogleLogin } from "./use-account-google-login";
import { useAccountVerification, type AccountChannel } from "./use-account-verification";
import { useMobileSettingsSubview } from "../settings/use-mobile-settings-subview";

export type {
  AccountUser,
  AccountProviders,
  AccountChallenge,
  AccountProfile,
  AccountPreferenceValue,
  AccountPreferences,
  AccountPreferenceSchema,
  AppIconInfo,
  AppIconClient,
  SettingsSyncClient,
  AccountClient,
  AccountCommunityDestination,
} from "./account-model";

type Confirmation = "logout-all" | "delete" | null;

/** 触屏宿主在「我的」里推入的页面；鸿蒙手机还会推入「其他平台下载」。它们记在历史条目的 `accountSubpage` 里，所以系统返回手势能关掉它们。 */
type AccountSubpage = "profile" | "download";

function accountSubpageOf(state: unknown): AccountSubpage | null {
  const value =
    state && typeof state === "object"
      ? (state as { accountSubpage?: unknown }).accountSubpage
      : null;
  return value === "profile" || value === "download" ? value : null;
}

/** 鸿蒙手机「我的」页上的一组：可选的 14px 标题，下面是一张装着若干 `NavRow` 的无边框卡片。 */
function MeGroup({ title, children }: { title?: string; children: ReactNode }) {
  const titleId = useId();
  return (
    <section className={account.meGroup} aria-labelledby={title ? titleId : undefined}>
      {title && (
        <h3 id={titleId} className={account.meGroupTitle}>
          {title}
        </h3>
      )}
      <NavGroup>{children}</NavGroup>
    </section>
  );
}

/** One row of a 我的 group on a touch host: it opens a page, a panel or a link. */
function MeRow({
  title,
  disabled,
  onClick,
}: {
  title: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={doc.linkRow} disabled={disabled} onClick={onClick}>
      <span className={doc.linkTitle}>{title}</span>
      <span aria-hidden="true">›</span>
    </button>
  );
}

/** The last, untitled group of 我的 on a touch host (dc.html `meGroups`): the walkthrough again, 帮助与反馈 and 关于. It does not depend on the account, so a host without one still reaches them. */
function MeSupportGroup({
  disabled,
  onReplayOnboarding,
  onOpenFeedback,
  onOpenAbout,
}: {
  disabled?: boolean;
  onReplayOnboarding?: () => void;
  onOpenFeedback?: () => void;
  onOpenAbout?: () => void;
}) {
  if (!onReplayOnboarding && !onOpenFeedback && !onOpenAbout) return null;
  return (
    <GroupList>
      {onReplayOnboarding && (
        <MeRow title="新手引导" disabled={disabled} onClick={onReplayOnboarding} />
      )}
      {onOpenFeedback && <MeRow title="帮助与反馈" disabled={disabled} onClick={onOpenFeedback} />}
      {onOpenAbout && <MeRow title="关于" disabled={disabled} onClick={onOpenAbout} />}
    </GroupList>
  );
}

export function AccountPage({
  client,
  appIcon,
  platform,
  mobile,
  onCancelLogin,
  onLoginComplete,
  onOpenPublishedSkins,
  onOpenLocalDesigns,
  onOpenCommunity,
  communityHasSkins,
  onOpenCloudDictionary,
  onOpenCloudClipboard,
  onOpenAbout,
  onOpenFeedback,
  onOpenDesktopDownload,
  onReplayOnboarding,
  ...meProps
}: {
  client?: AccountClient;
  appIcon?: AppIconClient;
  platform?: "android" | "ios" | "harmony";
  /** Host form factor, when a platform such as HarmonyOS has both touch and desktop hosts. */
  mobile?: boolean;
  onCancelLogin?: () => void;
  onLoginComplete?: () => void;
  onOpenPublishedSkins?: () => void;
  onOpenLocalDesigns?: () => void;
  onOpenCommunity?: (destination: AccountCommunityDestination) => void;
  /** 社区页里有没有皮肤画廊。没有时（Windows 的社区页只有词包与回复模板）「我发布的皮肤」不经社区页打开。缺省为有。 */
  communityHasSkins?: boolean;
  onOpenCloudDictionary?: () => void;
  onOpenCloudClipboard?: () => void;
  onOpenAbout?: () => void;
  /** Opens 反馈 (with 使用帮助 under it). A touch host passes it: 我的 is where the design keeps 帮助与反馈. */
  onOpenFeedback?: () => void;
  onOpenDesktopDownload?: () => void;
  onReplayOnboarding?: () => void;
} & HarmonyMeProps) {
  const resolvedAppIcon = appIcon ?? client?.appIcon;
  if (!client) {
    return (
      <div className={account.page}>
        {resolvedAppIcon && (
          <AppIconSettingsCard
            client={resolvedAppIcon}
            platform={platform === "harmony" ? undefined : platform}
          />
        )}
        <MeSupportGroup
          onReplayOnboarding={onReplayOnboarding}
          onOpenFeedback={onOpenFeedback}
          onOpenAbout={onOpenAbout}
        />
      </div>
    );
  }
  return (
    <AccountDetailsPage
      client={client}
      appIcon={resolvedAppIcon}
      platform={platform}
      mobile={mobile}
      onCancelLogin={onCancelLogin}
      onLoginComplete={onLoginComplete}
      onOpenPublishedSkins={onOpenPublishedSkins}
      onOpenLocalDesigns={onOpenLocalDesigns}
      onOpenCommunity={onOpenCommunity}
      communityHasSkins={communityHasSkins}
      onOpenCloudDictionary={onOpenCloudDictionary}
      onOpenCloudClipboard={onOpenCloudClipboard}
      onOpenAbout={onOpenAbout}
      onOpenFeedback={onOpenFeedback}
      onOpenDesktopDownload={onOpenDesktopDownload}
      onReplayOnboarding={onReplayOnboarding}
      {...meProps}
    />
  );
}

/** 鸿蒙手机「我的」页所需的附加能力；其他触屏宿主也可复用其中的通用入口。 */
type HarmonyMeProps = {
  /** 已保存的偏好；保留宿主接线兼容性，鸿蒙手机「我的」页不再重复展示设置入口。 */
  preferences?: Preferences;
  /** 打开共享设置页，例如「隐私」。 */
  onOpenPage?: (id: SettingsPageId) => void;
  /** 应用主题，用于「应用主题」行及其面板。 */
  appTheme?: AppThemeClient;
  /** 在系统浏览器里打开网页：隐私政策、下载页。 */
  onOpenUrl?: (url: string) => void;
  copyText?: (text: string) => Promise<void>;
  /** 所有平台共用的那一个下载页，由「其他平台下载」打开。 */
  desktopDownloadUrl?: string;
  /** 「关于」行显示的版本号，与「关于」页读到的一致。 */
  appVersion?: string;
};

function AccountDetailsPage({
  client,
  appIcon,
  platform,
  mobile: mobileOverride,
  onCancelLogin,
  onLoginComplete,
  onOpenPublishedSkins,
  onOpenLocalDesigns,
  onOpenCommunity,
  communityHasSkins = true,
  onOpenCloudDictionary,
  onOpenCloudClipboard,
  onOpenAbout,
  onOpenFeedback,
  onOpenDesktopDownload,
  onReplayOnboarding,
  preferences: _preferences,
  onOpenPage,
  appTheme,
  onOpenUrl,
  copyText,
  desktopDownloadUrl,
  appVersion,
}: HarmonyMeProps & {
  client: AccountClient;
  appIcon?: AppIconClient;
  platform?: "android" | "ios" | "harmony";
  mobile?: boolean;
  onCancelLogin?: () => void;
  onLoginComplete?: () => void;
  onOpenPublishedSkins?: () => void;
  onOpenLocalDesigns?: () => void;
  onOpenCommunity?: (destination: AccountCommunityDestination) => void;
  communityHasSkins?: boolean;
  /** 触屏宿主的「我的」里打开云词库。桌面的云词库入口在「词库」页，账号页不再列出。 */
  onOpenCloudDictionary?: () => void;
  onOpenCloudClipboard?: () => void;
  onOpenAbout?: () => void;
  /** Opens 反馈 (with 使用帮助 under it). A touch host passes it: 我的 is where the design keeps 帮助与反馈. */
  onOpenFeedback?: () => void;
  onOpenDesktopDownload?: () => void;
  onReplayOnboarding?: () => void;
}) {
  const mobile =
    mobileOverride ?? (platform === "android" || platform === "ios" || platform === "harmony");
  // 鸿蒙手机画重新设计的「我的」；它的 2-in-1 保留桌面页，其他触屏宿主保留各自原来的页面。
  const harmony = mobile && platform === "harmony";
  const showToast = useToast();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [providers, setProviders] = useState<AccountProviders>({ email: false, phone: false });
  const [providersLoaded, setProvidersLoaded] = useState(false);
  const [user, setUser] = useState<AccountUser | null>(null);
  // 读不出本机保存的登录状态（`status()` 失败，例如登录状态文件坏了）。只有这时才给「清除失效登录状态」，平常未登录时它只是噪音。
  const [sessionUnreadable, setSessionUnreadable] = useState(false);
  const [profile, setProfile] = useState<AccountProfile | null>(null);
  const [name, setName] = useState("");
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [editingProfile, setEditingProfile] = useState(false);
  const {
    value: mobileSubpage,
    open: openMobileSubpage,
    close: closeMobileSubpage,
    reset: resetMobileSubpage,
  } = useMobileSettingsSubview<AccountSubpage>({
    mobile,
    initialValue:
      mobile && typeof window !== "undefined" ? accountSubpageOf(window.history.state) : null,
    stateKey: "accountSubpage",
    decode: (state) => (state?.msimeSettings === true ? accountSubpageOf(state) : null),
    encode: (value) => ({ page: "account", accountSubpage: value }),
  });
  const [loginOpen, setLoginOpen] = useState(false);
  const [copiedAccountId, showCopiedAccountId] = useTemporaryFlag(1800);
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setError,
    setNotice,
  );
  const clearSession = () => {
    setUser(null);
    setProfile(null);
    setName("");
  };
  const { signOut, deleteAccount } = useAccountSessionActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onSignedOut: (all) => {
      clearSession();
      setConfirmation(null);
      setNotice(all ? "已退出所有设备。" : "已退出登录。");
    },
    onDeleted: (voiceSamplesKept) => {
      clearSession();
      setConfirmation(null);
      setNotice(voiceSamplesKept ? ACCOUNT_DELETED_VOICE_SAMPLES_KEPT : "账号已注销。");
    },
  });
  const applyProviders = (value: AccountProviders) => {
    setProviders(value);
    setProvidersLoaded(true);
  };
  const { refreshProviders: reloadProviders, clearExpired } = useAccountRecoveryActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onProvidersChange: applyProviders,
    onClearedExpired: () => {
      clearSession();
      setSessionUnreadable(false);
      setNotice("已清除失效登录状态。");
    },
  });
  const refreshProviders = () => {
    if (busy) return;
    setProvidersLoaded(false);
    return reloadProviders();
  };
  const verification = useAccountVerification({ client, mounted, clientGeneration, setNotice });
  const {
    channel,
    setChannel,
    setCode,
    setChallenge,
    selectChannel,
    sendCode,
    signIn: runVerificationSignIn,
  } = verification;
  const {
    waiting: googleWaiting,
    cancel: cancelGoogle,
    signIn: runGoogleLogin,
  } = useAccountGoogleLogin({ client, mounted, clientGeneration });

  // Leaving the page must not leave the loopback listener waiting for a browser the user abandoned.
  useEffect(() => () => cancelGoogle(), [client]);

  const openSubpage = openMobileSubpage;
  const closeSubpage = closeMobileSubpage;

  const applyProfile = (value: AccountProfile) => {
    setProfile(value);
    setUser(value.user);
    setName(value.user.displayName);
  };
  const { normalizedName, validName, rename } = useAccountRename({
    client,
    name,
    mounted,
    clientGeneration,
    perform,
    onRenamed: (updated) => {
      applyProfile(updated);
      setNotice("昵称已更新。");
    },
  });

  // 登录成功（任何一种方式）或者读出了已登录的用户，说明保存的登录状态是可读的，之前那次读不出来的结论作废；否则退出登录后又会冒出「清除失效登录状态」。
  useEffect(() => {
    if (user) setSessionUnreadable(false);
  }, [user]);

  const loadProfile = async (generation = clientGeneration.current) => {
    const value = await client.profile();
    if (mounted.current && generation === clientGeneration.current) applyProfile(value);
  };

  useEffect(() => {
    const generation = clientGeneration.current;
    setLoading(true);
    setProviders({ email: false, phone: false });
    setProvidersLoaded(false);
    void Promise.allSettled([client.status(), client.providers()]).then(async (results) => {
      if (!mounted.current || generation !== clientGeneration.current) return;
      const [status, available] = results;
      if (available.status === "fulfilled") applyProviders(available.value);
      else if (!isAccountCancellation(available.reason)) setError(accountMessage(available.reason));
      if (status.status === "fulfilled") {
        setSessionUnreadable(false);
        const nextUser = status.value.user ?? null;
        setUser(nextUser);
        if (nextUser) {
          setName(nextUser.displayName);
          try {
            const value = await client.profile();
            if (mounted.current && generation === clientGeneration.current) applyProfile(value);
          } catch (profileError) {
            if (
              mounted.current &&
              generation === clientGeneration.current &&
              !isAccountCancellation(profileError)
            )
              setError(accountMessage(profileError));
          }
        }
      } else {
        if (!isAccountCancellation(status.reason)) {
          setError(accountMessage(status.reason));
          setSessionUnreadable(true);
        }
      }
      if (mounted.current && generation === clientGeneration.current) setLoading(false);
    });
  }, [client, clientGeneration, mounted]);

  const chooseChannel = (value: AccountChannel) => {
    cancelGoogle();
    selectChannel(value);
    setError("");
    setNotice("");
  };

  const requestCode = () => void perform(sendCode);

  const signIn = () =>
    void perform(async () => {
      const result = await runVerificationSignIn();
      if (!result) return;
      setUser(result.user);
      setChannel(null);
      setChallenge(null);
      setCode("");
      await loadProfile(result.generation);
      if (!mounted.current || result.generation !== clientGeneration.current) return;
      setNotice("登录成功。");
      onLoginComplete?.();
    });

  // Only the user and profile change: the nickname field keeps whatever is being typed in the dialog.
  const applyAvatar = (updated: AccountProfile) => {
    setProfile(updated);
    setUser(updated.user);
  };

  const chooseAvatar = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      let updated: AccountProfile | null | undefined;
      try {
        updated = await client.chooseAvatar?.();
      } catch (error) {
        // The host refuses a file that is not a small PNG or JPEG as an invalid request; say what was wrong with it rather than with "the input".
        if (errorCode(error) === "account_invalid") throw { code: "account_avatar_invalid" };
        throw error;
      }
      if (!updated || !mounted.current || generation !== clientGeneration.current) return;
      applyAvatar(updated);
      setNotice("头像已更新。");
    });

  const removeAvatar = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!client.removeAvatar) return;
      const updated = await client.removeAvatar();
      if (!mounted.current || generation !== clientGeneration.current) return;
      applyAvatar(updated);
      setNotice("已移除头像。");
    });

  const copyAccountId = () => {
    copyAccountIdToClipboard({
      id: user?.id,
      isMounted: () => mounted.current,
      onCopied: showCopiedAccountId,
      setError,
    });
  };

  const openPublishedSkins =
    onOpenCommunity && communityHasSkins
      ? () => onOpenCommunity("published-skins")
      : onOpenPublishedSkins;

  if (loading)
    return (
      <div className={account.page}>
        <StatusMessage role="status">正在读取账号状态…</StatusMessage>
      </div>
    );

  if (harmony && mobileSubpage === "profile" && user)
    return (
      <HarmonyProfilePage
        client={client}
        user={user}
        profile={profile}
        onBack={closeSubpage}
        onSignedOut={(message) => {
          resetMobileSubpage();
          setUser(null);
          setProfile(null);
          setName("");
          showToast(message);
          if (typeof window !== "undefined") window.history.back();
        }}
        onProfileUpdated={applyProfile}
      />
    );

  if (harmony && mobileSubpage === "download" && desktopDownloadUrl)
    return (
      <MobileDownloadPage
        url={desktopDownloadUrl}
        onBack={closeSubpage}
        onOpenUrl={onOpenUrl}
        copyText={copyText}
      />
    );

  if (mobileSubpage === "profile" && user)
    return (
      <MobileAccountProfilePage
        client={client}
        user={user}
        profile={profile}
        onBack={closeSubpage}
        onSignedOut={() => {
          resetMobileSubpage();
          setUser(null);
          setProfile(null);
          setName("");
          if (typeof window !== "undefined") window.history.back();
        }}
        onProfileUpdated={applyProfile}
      />
    );

  // Count only providers this host can render; the backend may enable Apple or Google for hosts without a native client for them.
  const appleAvailable = providers.apple === true && Boolean(client.appleLogin);
  const googleAvailable = providers.google === true && Boolean(client.googleLogin);
  const enabledProviders =
    Number(providers.email) +
    Number(providers.phone) +
    Number(appleAvailable) +
    Number(googleAvailable);

  const signInWithApple = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!client.appleLogin) throw { code: "account_unavailable" };
      const result = await client.appleLogin();
      if (!mounted.current || generation !== clientGeneration.current) return;
      if (!result.user) throw { code: "account_unavailable" };
      setUser(result.user);
      await loadProfile(generation);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice("登录成功。");
      onLoginComplete?.();
    });

  const signInWithGoogle = () =>
    void perform(async () => {
      const result = await runGoogleLogin();
      if (!result) return;
      setUser(result.user);
      await loadProfile(result.generation);
      if (!mounted.current || result.generation !== clientGeneration.current) return;
      setNotice("登录成功。");
      onLoginComplete?.();
    });

  if (harmony) {
    // 别的页面请求了登录（`onCancelLogin` 是回去的路），所以面板直接打开；关掉它就回到那里。
    const loginShown = !user && (loginOpen || Boolean(onCancelLogin));
    const closeLogin = () => {
      setLoginOpen(false);
      onCancelLogin?.();
    };
    const signedIn = (nextUser: AccountUser) => {
      const generation = clientGeneration.current;
      setUser(nextUser);
      setName(nextUser.displayName);
      setLoginOpen(false);
      showToast("已登录");
      void perform(() => loadProfile(generation));
      onLoginComplete?.();
    };
    const communityRows = Boolean(
      onOpenLocalDesigns || (user && (openPublishedSkins || onOpenCommunity)),
    );
    const downloadPage = Boolean(desktopDownloadUrl && (onOpenUrl || copyText));
    const openDownload = downloadPage ? () => openSubpage("download") : onOpenDesktopDownload;
    const icon = (name: FluentIconName) => <FluentIcon name={name} size={18} />;
    return (
      <div className={account.mePage}>
        <AccountStatusMessages
          error={error}
          notice={notice}
          noticeClassName={`notice ${account.status}`}
        />
        <button
          type="button"
          className={account.meCard}
          disabled={busy}
          aria-label={user ? "编辑个人资料" : "未登录，点按登录"}
          onClick={() => (user ? openSubpage("profile") : setLoginOpen(true))}
        >
          {user ? (
            <AccountAvatar user={user} load={client.avatar} size="card" />
          ) : (
            <span className={account.meCardAnonymous} aria-hidden="true">
              ?
            </span>
          )}
          <span className={account.meCardText}>
            <span className={account.meCardName}>
              {user ? preferredAccountName(user) : "未登录"}
            </span>
            <span className={account.meCardSubtitle}>
              {user ? (user.email ?? "水杉账号已登录") : "登录后同步词库、皮肤和设置"}
            </span>
          </span>
          <FluentIcon name="chevron_right" size={20} className={account.meCardChevron} />
        </button>
        {appIcon && <AppIconSettingsCard client={appIcon} />}
        {communityRows && (
          <MeGroup title="社区作品">
            {onOpenLocalDesigns && (
              <NavRow
                variant="me"
                icon={icon("pen")}
                title="我的设计"
                disabled={busy}
                onClick={onOpenLocalDesigns}
              />
            )}
            {user && openPublishedSkins && (
              <NavRow
                variant="me"
                icon={icon("people_community")}
                title="我发布的皮肤"
                disabled={busy}
                onClick={openPublishedSkins}
              />
            )}
            {user && onOpenCommunity && (
              <>
                <NavRow
                  variant="me"
                  icon={icon("book")}
                  title="我发布的词库"
                  disabled={busy}
                  onClick={() => onOpenCommunity("published-dictionary")}
                />
                <NavRow
                  variant="me"
                  icon={icon("text_quote")}
                  title="我发布的回复模板"
                  disabled={busy}
                  onClick={() => onOpenCommunity("published-reply")}
                />
                <NavRow
                  variant="me"
                  icon={icon("star")}
                  title="收藏的词库"
                  disabled={busy}
                  onClick={() => onOpenCommunity("saved-dictionary")}
                />
                <NavRow
                  variant="me"
                  icon={icon("heart")}
                  title="收藏的回复模板"
                  disabled={busy}
                  onClick={() => onOpenCommunity("saved-reply")}
                />
              </>
            )}
          </MeGroup>
        )}
        {/* 「绑定 QQ 群助手」：/bind 拿到的码多半就在这台手机的 QQ 里。 */}
        {user && client.qqLinks && <QqLinkCard client={client.qqLinks} userId={user.id} />}
        {(appTheme || onOpenPage || onOpenUrl) && (
          <MeGroup title="通用">
            {appTheme && <AppThemeRow client={appTheme} disabled={busy} />}
            {(onOpenPage || onOpenUrl) && (
              <NavRow
                variant="me"
                icon={icon("shield_lock")}
                title="隐私"
                value="本地优先"
                disabled={busy}
                // 打开应用里的「隐私」页（数据怎么处理、使用统计开关），完整的隐私政策网页在那一页的最后一行；没有设置页可开的宿主才直接打开网页。
                onClick={() => (onOpenPage ? onOpenPage("privacy") : onOpenUrl?.(privacyUrl))}
              />
            )}
          </MeGroup>
        )}
        {(openDownload || onOpenFeedback || onOpenAbout || onReplayOnboarding) && (
          <MeGroup>
            {openDownload && (
              <NavRow
                variant="me"
                icon={icon("arrow_download")}
                title="其他平台下载"
                value={downloadPage ? `${otherPlatformCount} 个平台` : undefined}
                disabled={busy}
                onClick={openDownload}
              />
            )}
            {onOpenFeedback && (
              <NavRow
                variant="me"
                icon={icon("person_feedback")}
                title="帮助与反馈"
                disabled={busy}
                onClick={onOpenFeedback}
              />
            )}
            {onOpenAbout && (
              <NavRow
                variant="me"
                icon={icon("info")}
                title="关于"
                value={appVersion}
                disabled={busy}
                onClick={onOpenAbout}
              />
            )}
            {onReplayOnboarding && (
              <NavRow
                variant="me"
                icon={icon("keyboard")}
                title="新手引导"
                disabled={busy}
                onClick={onReplayOnboarding}
              />
            )}
          </MeGroup>
        )}
        {loginShown && (
          <LoginSheet
            client={client}
            providers={providers}
            onProvidersChange={applyProviders}
            onSignedIn={signedIn}
            onClearedExpired={() => {
              setUser(null);
              setProfile(null);
              setName("");
              showToast("已清除失效登录状态");
            }}
            onClose={closeLogin}
            onOpenUrl={onOpenUrl}
          />
        )}
      </div>
    );
  }

  return (
    <div className={account.page}>
      {!user && onCancelLogin && (
        <div className={account.profilePageHeader}>
          <ActionButton
            action={() => {
              cancelGoogle();
              onCancelLogin();
            }}
            className="secondary"
            disabled={busy && !(googleWaiting && client.googleCancel)}
            label="取消"
          />
          <h2 className={account.heading}>登录水杉</h2>
        </div>
      )}
      <AccountStatusMessages
        error={error}
        notice={notice}
        noticeClassName={`notice ${account.status}`}
      />
      {user ? (
        <button
          type="button"
          className={`${account.section} ${account.hero} ${account.profileCard}`}
          disabled={busy}
          aria-label="编辑个人资料"
          onClick={() => {
            if (mobile && typeof window !== "undefined") {
              openSubpage("profile");
            } else {
              setName(user.displayName);
              setEditingProfile(true);
            }
          }}
        >
          <AccountAvatar user={user} load={client.avatar} size="medium" />
          <div>
            <h2 className={account.heading}>{preferredAccountName(user)}</h2>
            <p className={account.note}>{user.email ?? "水杉账号已登录"}</p>
          </div>
          <span className={account.profileChevron} aria-hidden="true">
            ›
          </span>
        </button>
      ) : (
        <section className={`${account.section} ${account.signIn}`}>
          <div className={account.signInHeader}>
            <div className={account.avatar("large")} aria-hidden="true">
              杉
            </div>
            <div>
              <h2 className={account.heading}>
                {channel === "email"
                  ? "邮箱登录"
                  : channel === "phone"
                    ? "手机号登录"
                    : "欢迎来到水杉"}
              </h2>
              <p className={account.note}>
                {channel
                  ? "我们会发送一个 6 位验证码完成登录"
                  : mobile
                    ? "登录，分享你的键盘设计"
                    : // 设置同步只接在 Windows 和 Linux 的账号会话上（`settingsSync`），别的桌面不许诺同步设置。
                      client.settingsSync
                      ? "登录后在设备之间同步设置和词库，还可以发布你的候选窗口皮肤"
                      : "登录后在设备之间同步词库，还可以发布你的候选窗口皮肤"}
              </p>
            </div>
          </div>
          {!channel ? (
            <>
              <div className={account.signInBody}>
                {appleAvailable && (
                  <ActionButton
                    action={signInWithApple}
                    className={account.provider}
                    disabled={busy}
                    label="使用 Apple 登录"
                  />
                )}
                {googleAvailable && (
                  <ActionButton
                    action={signInWithGoogle}
                    className={account.provider}
                    disabled={busy}
                    label={googleWaiting ? "正在等待浏览器完成 Google 登录…" : "使用 Google 登录"}
                  />
                )}
                {googleWaiting && client.googleCancel && (
                  <ActionButton
                    action={cancelGoogle}
                    className={account.link}
                    label="取消 Google 登录"
                  />
                )}
                {providers.email && (
                  <ActionButton
                    action={() => chooseChannel("email")}
                    className={account.provider}
                    label="邮箱登录"
                  />
                )}
                {providers.phone && (
                  <ActionButton
                    action={() => chooseChannel("phone")}
                    className={account.provider}
                    label="手机号登录"
                  />
                )}
                {enabledProviders === 0 && (
                  <p className={`${account.muted} text-center`}>
                    {providersLoaded
                      ? "当前没有可用的验证码登录方式，请稍后重试。"
                      : busy
                        ? "正在加载登录方式…"
                        : "登录方式加载失败，请重试。"}
                  </p>
                )}
              </div>
              {/* 两个恢复动作只在出了问题时出现：登录方式只在进入本页时读一次，读失败或一个都没有时（上面那句「请稍后重试」）才给「刷新登录方式」；读不出本机登录状态时才给「清除失效登录状态」。 */}
              {(enabledProviders === 0 || sessionUnreadable) && (
                <div className={account.signInFooter}>
                  {enabledProviders === 0 && (
                    <ActionButton
                      action={refreshProviders}
                      className={account.link}
                      disabled={busy}
                      label="刷新登录方式"
                    />
                  )}
                  {sessionUnreadable && (
                    <ActionButton
                      action={clearExpired}
                      className={account.link}
                      disabled={busy}
                      label="清除失效登录状态"
                    />
                  )}
                </div>
              )}
            </>
          ) : (
            <div className={account.signInBody}>
              <AccountVerificationForm
                variant="page"
                verification={verification}
                busy={busy}
                onRequestCode={requestCode}
                onSignIn={signIn}
              />
              <div className={account.signInFooter}>
                <ActionButton
                  action={() => setChannel(null)}
                  className={account.link}
                  disabled={busy}
                  label="取消"
                />
              </div>
            </div>
          )}
        </section>
      )}
      {appIcon && (
        <AppIconSettingsCard
          client={appIcon}
          platform={platform === "harmony" ? undefined : platform}
        />
      )}
      {user && !mobile && editingProfile && (
        <div
          className={account.modalBackdrop}
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !busy) setEditingProfile(false);
          }}
        >
          <section
            className={account.modal}
            role="dialog"
            aria-modal="true"
            aria-label="编辑个人资料"
          >
            <div className={account.modalHeading}>
              <h2 className={account.heading}>编辑资料</h2>
              <ActionButton
                action={() => setEditingProfile(false)}
                className="secondary"
                disabled={busy}
                label="关闭"
              />
            </div>
            <div className={account.profilePreview}>
              {client.chooseAvatar ? (
                <button
                  type="button"
                  className={account.avatarButton}
                  disabled={busy}
                  aria-label="更换头像"
                  title="更换头像"
                  onClick={chooseAvatar}
                >
                  <AccountAvatar user={user} load={client.avatar} size="small" />
                </button>
              ) : (
                <AccountAvatar user={user} load={client.avatar} size="small" />
              )}
              <strong>{name.trim() || "你的昵称"}</strong>
              {user.avatarUploaded && client.removeAvatar && (
                <ActionButton
                  action={removeAvatar}
                  className={account.link}
                  disabled={busy}
                  label="移除头像"
                />
              )}
            </div>
            {client.chooseAvatar && (
              <p className={account.muted}>点头像可更换，支持 1 MiB 以内的 PNG 或 JPEG。</p>
            )}
            <AccountNicknameEditor
              variant="desktop"
              value={name}
              disabled={busy}
              normalizedName={normalizedName}
              validName={validName}
              onChange={setName}
            />
            <AccountIdentityDetails
              user={user}
              providers={profile?.providers ?? []}
              copied={copiedAccountId}
              onCopy={copyAccountId}
            />
            <div className={account.actionRow}>
              <ActionButton
                action={() => {
                  rename();
                  setEditingProfile(false);
                }}
                className={account.primary}
                disabled={busy || normalizedName === user.displayName}
                label="保存修改"
              />
              <ActionButton
                action={() => setEditingProfile(false)}
                className="secondary"
                disabled={busy}
                label="取消"
              />
            </div>
          </section>
        </div>
      )}
      {user && !mobile && client.settingsSync && (
        <SettingsSyncCard client={client.settingsSync} userId={user.id} />
      )}
      {user && !mobile && client.qqLinks && <QqLinkCard client={client.qqLinks} userId={user.id} />}
      {/* 桌面的云词库入口只在「词库」页：那里不登录也看得到，这里不再放第二个。macOS 不在设置里打开云剪贴板（要输入法提供输入会话），这一组在 macOS 上因此整组不出现。 */}
      {user && !mobile && onOpenCloudClipboard && (
        <GroupList title="云端">
          <MeRow title="云剪贴板" disabled={busy} onClick={onOpenCloudClipboard} />
        </GroupList>
      )}
      {!mobile && (onOpenLocalDesigns || (user && (openPublishedSkins || onOpenCommunity))) && (
        <GroupList title="我的内容">
          {onOpenLocalDesigns && (
            <Row title="我的设计" description="保存在本机的键盘皮肤，不会因登录账号而上传。">
              <ActionButton
                action={onOpenLocalDesigns}
                className={account.rowButton}
                disabled={busy}
                label="打开设计器"
              />
            </Row>
          )}
          {user && openPublishedSkins && (
            <MeRow title="我发布的皮肤" disabled={busy} onClick={openPublishedSkins} />
          )}
          {user && onOpenCommunity && (
            <>
              <MeRow
                title="我发布的词库"
                disabled={busy}
                onClick={() => onOpenCommunity("published-dictionary")}
              />
              <MeRow
                title="我发布的回复模板"
                disabled={busy}
                onClick={() => onOpenCommunity("published-reply")}
              />
              <MeRow
                title="收藏的词库"
                disabled={busy}
                onClick={() => onOpenCommunity("saved-dictionary")}
              />
              <MeRow
                title="收藏的回复模板"
                disabled={busy}
                onClick={() => onOpenCommunity("saved-reply")}
              />
            </>
          )}
        </GroupList>
      )}
      {user && !mobile && (
        <GroupList title="账号">
          <Row
            title="这台设备"
            description="退出后，设置同步、云词库、云剪贴板和发布作品都需要重新登录才能使用。"
          >
            <ActionButton
              action={() => signOut(false)}
              className={account.rowButton}
              disabled={busy}
              label="退出登录"
            />
          </Row>
          <Row title="所有设备" description="所有已登录的设备都需要重新登录。">
            <ActionButton
              action={() => setConfirmation("logout-all")}
              className={account.rowButton}
              disabled={busy}
              label="退出所有设备"
            />
          </Row>
          <Row title="注销账号" description="删除账号及已发布的作品、评分等云端数据，无法撤销。">
            <ActionButton
              action={() => setConfirmation("delete")}
              className={account.rowDanger}
              disabled={busy}
              label="注销账号"
            />
          </Row>
          {confirmation && (
            <div className={account.rowBlock}>
              <AccountConfirmation
                action={confirmation}
                busy={busy}
                onConfirm={() => {
                  if (confirmation)
                    runAccountSessionAction(confirmation, { signOut, deleteAccount });
                }}
                onCancel={() => setConfirmation(null)}
              />
            </div>
          )}
        </GroupList>
      )}
      {mobile && (
        // The design's 我的 groups (dc.html `meGroups`), less the rows with nothing behind them (我的设备, 隐私, 开屏动画) and the ones the 设置 list already holds (词库, 自造词). The account's own settings stay on the profile page behind the card above.
        <>
          {((user && (onOpenCloudClipboard || onOpenCloudDictionary)) || onOpenDesktopDownload) && (
            <GroupList title="工具">
              {user && onOpenCloudClipboard && (
                <MeRow title="云剪贴板" disabled={busy} onClick={onOpenCloudClipboard} />
              )}
              {user && onOpenCloudDictionary && (
                <MeRow title="云词库" disabled={busy} onClick={onOpenCloudDictionary} />
              )}
              {onOpenDesktopDownload && (
                <MeRow title="其他平台下载" disabled={busy} onClick={onOpenDesktopDownload} />
              )}
            </GroupList>
          )}
          {user && client.settingsSync && (
            <SettingsSyncCard client={client.settingsSync} userId={user.id} />
          )}
          {/* 「绑定 QQ 群助手」在手机上同样要有：/bind 拿到的码可能就在手机的 QQ 里。 */}
          {user && client.qqLinks && <QqLinkCard client={client.qqLinks} userId={user.id} />}
          {(onOpenLocalDesigns || (user && (openPublishedSkins || onOpenCommunity))) && (
            <GroupList title="我的内容">
              {onOpenLocalDesigns && (
                <MeRow title="我的设计" disabled={busy} onClick={onOpenLocalDesigns} />
              )}
              {user && openPublishedSkins && (
                <MeRow title="我发布的皮肤" disabled={busy} onClick={openPublishedSkins} />
              )}
              {user && onOpenCommunity && (
                <>
                  <MeRow
                    title="我发布的词库"
                    disabled={busy}
                    onClick={() => onOpenCommunity("published-dictionary")}
                  />
                  <MeRow
                    title="我发布的回复模板"
                    disabled={busy}
                    onClick={() => onOpenCommunity("published-reply")}
                  />
                  <MeRow
                    title="收藏的词库"
                    disabled={busy}
                    onClick={() => onOpenCommunity("saved-dictionary")}
                  />
                  <MeRow
                    title="收藏的回复模板"
                    disabled={busy}
                    onClick={() => onOpenCommunity("saved-reply")}
                  />
                </>
              )}
            </GroupList>
          )}
          <MeSupportGroup
            disabled={busy}
            onReplayOnboarding={onReplayOnboarding}
            onOpenFeedback={onOpenFeedback}
            onOpenAbout={onOpenAbout}
          />
        </>
      )}
      {!mobile && (onOpenAbout || onOpenDesktopDownload || onReplayOnboarding) && (
        <GroupList title="关于">
          {onOpenAbout && <MeRow title="关于水杉" disabled={busy} onClick={onOpenAbout} />}
          {onOpenDesktopDownload && (
            <MeRow title="电脑版下载" disabled={busy} onClick={onOpenDesktopDownload} />
          )}
          {onReplayOnboarding && (
            <MeRow title="重新查看新手引导" disabled={busy} onClick={onReplayOnboarding} />
          )}
        </GroupList>
      )}
    </div>
  );
}
