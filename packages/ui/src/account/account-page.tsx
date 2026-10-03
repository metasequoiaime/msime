import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { ActionButton } from "../core/action-button";
import { errorCode } from "../core/error-code";
import { GroupList, Row } from "../core/platform-controls";
import * as doc from "../settings/document-style";
import * as account from "./account-style";
import { AccountAvatar } from "./account-avatar";
import { preferredAccountName } from "./account-labels";
import { accountMessage, isAccountCancellation } from "./account-errors";
import { AccountConfirmation } from "./account-confirmation";
import { AccountNicknameField } from "./account-nickname-field";
import { AccountInputField } from "./account-input-field";
import { AccountStatusMessages } from "./account-status-messages";
import { AccountIdentityDetails } from "./account-identity-details";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { copyAccountId as copyAccountIdToClipboard } from "./account-id-copy";
import { runAccountOperation } from "./account-operation";

export type AccountUser = {
  id: string;
  displayName: string;
  createdAt: string;
  /** The verified email of a linked Google account. */
  email?: string;
  /** Changes whenever the avatar does; the image itself comes from `AccountClient.avatar`, since the page loads no remote image. */
  avatarUrl?: string;
  /** The avatar is one the user uploaded, which they can remove, rather than their Google picture. */
  avatarUploaded?: boolean;
};

export type AccountProviders = {
  email: boolean;
  phone: boolean;
  apple?: boolean;
  google?: boolean;
};

export type AccountChallenge = {
  challengeId: string;
  expiresIn: number;
};

export type AccountProfile = {
  user: AccountUser;
  providers: string[];
};

export type AccountPreferenceValue = boolean | number | string;

export type AccountPreferences = {
  revision: number;
  settings: Record<string, AccountPreferenceValue>;
};

export type AccountPreferenceSchema = {
  fields: Record<string, { type: string }>;
  maximumBytes: number;
  updateMode: string;
  revisionRequired: boolean;
};

export type AppIconInfo = {
  supported: boolean;
  selected: string;
};

export interface AppIconClient {
  info(): Promise<AppIconInfo>;
  set(style: string): Promise<AppIconInfo>;
}

export interface SettingsSyncClient {
  schema(): Promise<AccountPreferenceSchema>;
  load(): Promise<AccountPreferences>;
  upload(): Promise<AccountPreferences>;
  apply(userId: string, preferences: AccountPreferences): Promise<void>;
}

export interface AccountClient {
  status(): Promise<{ user?: AccountUser | null }>;
  providers(): Promise<AccountProviders>;
  requestCode(provider: "email" | "phone", target: string): Promise<AccountChallenge>;
  login(challengeId: string, code: string): Promise<{ user?: AccountUser | null }>;
  /** iOS performs the nonce and AuthenticationServices exchange natively. */
  appleLogin?: () => Promise<{ user?: AccountUser | null }>;
  /** Desktop hosts run the Google browser and loopback redirect natively; the page never sees the authorization code. */
  googleLogin?: () => Promise<{ user?: AccountUser | null }>;
  /** Ends a pending googleLogin, which then rejects as cancelled. The browser cannot report a closed Google tab, so this is how the user gives up without waiting for the timeout. */
  googleCancel?: () => Promise<void>;
  profile(): Promise<AccountProfile>;
  rename(displayName: string): Promise<AccountProfile>;
  /** The signed-in user's avatar as a `data:` URL, or null without one. Absent on hosts that do not fetch avatars, which show the name's first character. */
  avatar?: () => Promise<string | null>;
  /** Opens the platform's file dialog for a PNG or JPEG and uploads it; null when the user closes the dialog. The page never names a path. */
  chooseAvatar?: () => Promise<AccountProfile | null>;
  /** Removes the uploaded avatar; the Google picture, if any, shows again. */
  removeAvatar?: () => Promise<AccountProfile>;
  logout(all: boolean): Promise<void>;
  deleteAccount(): Promise<void>;
  clearExpired(): Promise<void>;
  settingsSync?: SettingsSyncClient;
  appIcon?: AppIconClient;
}

export type AccountCommunityDestination =
  | "published-skins"
  | "published-dictionary"
  | "saved-dictionary"
  | "published-reply"
  | "saved-reply";

type Channel = "email" | "phone";
type Confirmation = "logout-all" | "delete" | null;

function MobileAccountProfilePage({
  client,
  user,
  profile,
  onBack,
  onSignedOut,
  onProfileUpdated,
}: {
  client: AccountClient;
  user: AccountUser;
  profile: AccountProfile | null;
  onBack: () => void;
  onSignedOut: () => void;
  onProfileUpdated: (profile: AccountProfile) => void;
}) {
  const [name, setName] = useState(user.displayName);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmation, setConfirmation] = useState<
    "logout" | "logout-all" | "relogin" | "delete" | null
  >(null);
  const [copied, setCopied] = useState(false);
  const mounted = useRef(true);
  const clientGeneration = useRef(0);
  const actionRunning = useRef(false);
  const normalizedName = name.trim();
  const validName =
    Boolean(normalizedName) &&
    [...normalizedName].length <= 64 &&
    !/[\u0000-\u001f\u007f]/.test(normalizedName);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    actionRunning.current = false;
    setBusy(false);
    return () => {
      mounted.current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client]);

  const perform = async (operation: () => Promise<void>) => {
    if (actionRunning.current) return;
    const generation = clientGeneration.current;
    actionRunning.current = true;
    try {
      await runAccountOperation(
        {
          busy,
          isCurrent: () => mounted.current && generation === clientGeneration.current,
          setBusy,
          setError,
          setNotice,
        },
        operation,
      );
    } finally {
      if (generation === clientGeneration.current) actionRunning.current = false;
    }
  };
  const rename = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      if (!validName) throw { code: "account_invalid" };
      const updated = await client.rename(normalizedName);
      if (!mounted.current || generation !== clientGeneration.current) return;
      onProfileUpdated(updated);
      setName(updated.user.displayName);
      setNotice("昵称已更新。");
    });
  const signOut = (all: boolean) =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.logout(all);
      if (!mounted.current || generation !== clientGeneration.current) return;
      onSignedOut();
    });
  const clearExpired = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.clearExpired();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onSignedOut();
    });
  const deleteAccount = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.deleteAccount();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onSignedOut();
    });
  const confirmAction = () => {
    const action = confirmation;
    setConfirmation(null);
    if (action === "logout") signOut(false);
    else if (action === "logout-all") signOut(true);
    else if (action === "relogin") clearExpired();
    else if (action === "delete") deleteAccount();
  };
  const copyId = () => {
    copyAccountIdToClipboard({
      id: user.id,
      isMounted: () => mounted.current,
      setCopied,
      setError,
    });
  };

  return (
    <div className={account.page}>
      <div className={account.profilePageHeader}>
        <ActionButton action={onBack} className="secondary" disabled={busy} label="‹ 返回" />
        <h2 className={account.heading}>编辑资料</h2>
      </div>
      <AccountStatusMessages error={error} notice={notice} />
      <section className={`${account.section} ${account.profilePreviewLarge}`}>
        <AccountAvatar
          user={user}
          name={normalizedName || "水杉用户"}
          load={client.avatar}
          size="large"
        />
        <h2 className={account.heading}>{normalizedName || "你的昵称"}</h2>
        <p className={account.muted}>在水杉，留下你的名字</p>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>社区昵称</h2>
        <AccountNicknameField
          value={name}
          disabled={busy}
          ariaLabel="编辑社区昵称"
          onChange={setName}
        />
        <p className={`account-muted${!validName && normalizedName ? " error" : ""}`}>
          {normalizedName
            ? validName
              ? "昵称会显示在社区作品中，已发布的作品也会同步更新。"
              : "昵称最多 64 个字符，请勿使用换行或控制字符。"
            : "取一个喜欢的名字，让大家记住你。"}
        </p>
        <p className={account.muted}>{[...normalizedName].length}/64</p>
        <div className={account.actionRow}>
          <ActionButton
            action={rename}
            className={account.primary}
            disabled={busy || !validName || normalizedName === user.displayName}
            label="保存昵称"
          />
        </div>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>账号信息</h2>
        <AccountIdentityDetails
          user={user}
          providers={profile?.providers ?? []}
          copied={copied}
          onCopy={copyId}
        />
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>账号操作</h2>
        <div className={account.actionRow}>
          <ActionButton
            action={() => setConfirmation("logout")}
            className="secondary"
            disabled={busy}
            label="退出登录"
          />
          <ActionButton
            action={() => setConfirmation("logout-all")}
            className="secondary"
            disabled={busy}
            label="退出所有设备"
          />
          <ActionButton
            action={() => setConfirmation("relogin")}
            className="secondary"
            disabled={busy}
            label="重新登录"
          />
          <ActionButton
            action={() => setConfirmation("delete")}
            className="danger-text"
            disabled={busy}
            label="注销账号"
          />
        </div>
      </section>
      {confirmation && (
        <AccountConfirmation
          action={confirmation}
          busy={busy}
          confirmLabel="确认"
          onConfirm={confirmAction}
          onCancel={() => setConfirmation(null)}
        />
      )}
    </div>
  );
}

const appIconOptions = [
  { id: "classic", title: "原版", detail: "经典黑白，简洁如初", color: "#252525" },
  { id: "forest", title: "杉林", detail: "杉叶青绿，沉静自然", color: "#2f6b4f" },
  { id: "sky", title: "晴空", detail: "清透蓝调，轻盈明亮", color: "#4e8fc8" },
  { id: "dusk", title: "暮紫", detail: "晚霞淡紫，温柔入夜", color: "#71618f" },
  { id: "vermilion", title: "朱砂", detail: "朱红印记，纸上东方", color: "#b9473f" },
] as const;

function AppIconSettingsCard({
  client,
  platform,
}: {
  client: AppIconClient;
  platform?: "android" | "ios";
}) {
  const [info, setInfo] = useState<AppIconInfo | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState("");
  const mounted = useRef(true);
  const generation = useRef(0);
  const changeRunning = useRef(false);

  useEffect(() => {
    let active = true;
    const current = ++generation.current;
    mounted.current = true;
    changeRunning.current = false;
    setInfo(null);
    setError("");
    void client
      .info()
      .then((value) => {
        if (active && mounted.current && generation.current === current) setInfo(value);
      })
      .catch(() => {
        if (active && mounted.current && generation.current === current)
          setError("暂时无法读取 App 图标状态，请稍后重试。");
      });
    return () => {
      active = false;
      mounted.current = false;
    };
  }, [client]);

  const choose = async (style: string) => {
    if (!info?.supported || pending || changeRunning.current || info.selected === style) return;
    const current = generation.current;
    changeRunning.current = true;
    setPending(style);
    setError("");
    try {
      const updated = await client.set(style);
      if (!mounted.current || generation.current !== current) return;
      setInfo(updated);
      if (updated.selected !== style) setError("图标未能更换，请稍后重试。");
    } catch {
      // Android launchers and the iOS Simulator can report an error after
      // applying the icon. Read the OS state again before showing a failure.
      try {
        const updated = await client.info();
        if (!mounted.current || generation.current !== current) return;
        setInfo(updated);
        if (updated.selected !== style) setError("图标未能更换，请稍后重试。");
      } catch {
        if (mounted.current && generation.current === current)
          setError("图标未能更换，请稍后重试。");
      }
    } finally {
      if (generation.current === current) changeRunning.current = false;
      if (mounted.current && generation.current === current) setPending(null);
    }
  };

  return (
    <section className={`${account.section} ${account.iconSettings}`}>
      <div>
        <h2 className={account.heading}>App 图标</h2>
        <p className={account.note}>
          给主屏幕上的水杉换个颜色。
          {platform === "android"
            ? "Android 会使用系统启动器的图标别名保存选择。"
            : platform === "ios"
              ? "iOS 会使用系统备用图标接口保存选择。"
              : "系统会使用平台提供的图标切换能力保存选择。"}
        </p>
      </div>
      {info === null && !error && <p role="status">正在读取图标状态…</p>}
      <AccountStatusMessages error={error} />
      {info && !info.supported && <p className={account.muted}>当前设备暂不支持更换 App 图标。</p>}
      {info && (
        <div className={account.iconGrid}>
          {appIconOptions.map((option) => {
            const selected = info.selected === option.id;
            const changing = pending === option.id;
            return (
              <ActionButton
                action={() => void choose(option.id)}
                ariaLabel={`${option.title}，${option.detail}`}
                ariaPressed={selected}
                className={account.iconCard(selected)}
                key={option.id}
                disabled={!info.supported || pending !== null}
                label={
                  <>
                    <span
                      className={account.iconPreview}
                      style={{ backgroundColor: option.color }}
                      aria-hidden="true"
                    >
                      杉
                    </span>
                    <span className={account.iconCopy}>
                      <strong>{option.title}</strong>
                      <small>{option.detail}</small>
                    </span>
                    <span className={account.iconState(selected)}>
                      {changing ? "更换中" : selected ? "使用中" : "使用此图标"}
                    </span>
                  </>
                }
              />
            );
          })}
        </div>
      )}
      <p className={account.muted}>更换后，系统启动器可能需要片刻刷新。随时可以切回原版。</p>
    </section>
  );
}

function SettingsSyncCard({ client, userId }: { client: SettingsSyncClient; userId: string }) {
  const [schema, setSchema] = useState<AccountPreferenceSchema | null>(null);
  const [cloud, setCloud] = useState<AccountPreferences | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [confirmation, setConfirmation] = useState<"upload" | "apply" | null>(null);
  const mounted = useRef(true);
  const generation = useRef(0);
  const actionBusy = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const load = async () => {
    if (busy || !mounted.current || actionBusy.current) return;
    const current = generation.current;
    actionBusy.current = true;
    setBusy(true);
    setMessage("");
    try {
      const [nextSchema, nextCloud] = await Promise.all([client.schema(), client.load()]);
      if (!mounted.current || generation.current !== current) return;
      setSchema(nextSchema);
      setCloud(nextCloud);
    } catch (error) {
      if (mounted.current && generation.current === current && !isAccountCancellation(error))
        setMessage(accountMessage(error));
    } finally {
      if (mounted.current && generation.current === current) {
        actionBusy.current = false;
        setBusy(false);
      }
    }
  };

  useEffect(() => {
    let active = true;
    const current = ++generation.current;
    setSchema(null);
    setCloud(null);
    setMessage("");
    setBusy(true);
    void Promise.all([client.schema(), client.load()])
      .then(([nextSchema, nextCloud]) => {
        if (!active || !mounted.current || generation.current !== current) return;
        setSchema(nextSchema);
        setCloud(nextCloud);
      })
      .catch((error) => {
        if (
          active &&
          mounted.current &&
          generation.current === current &&
          !isAccountCancellation(error)
        )
          setMessage(accountMessage(error));
      })
      .finally(() => {
        if (active && mounted.current && generation.current === current) setBusy(false);
      });
    return () => {
      active = false;
      actionBusy.current = false;
    };
  }, [client, userId]);

  const runConfirmed = async () => {
    if (!cloud || !schema || !confirmation || busy || actionBusy.current) return;
    const current = generation.current;
    const operation = confirmation;
    setConfirmation(null);
    actionBusy.current = true;
    await runAsyncAction(
      {
        busy: false,
        isCurrent: () => mounted.current && generation.current === current,
        setBusy: (value) => {
          actionBusy.current = value;
          setBusy(value);
        },
        setError: setMessage,
      },
      async (isCurrent) => {
        if (operation === "upload") {
          const next = await client.upload();
          if (!isCurrent()) return;
          setCloud(next);
        } else {
          await client.apply(userId, cloud);
          if (!isCurrent()) return;
        }
        setMessage(
          operation === "upload"
            ? "本机设置已上传。"
            : "已应用云端设置。请重新打开键盘使部分设置生效。",
        );
      },
      { formatError: accountMessage, ignoreError: isAccountCancellation },
    );
    if (generation.current === current) actionBusy.current = false;
  };

  const hasCloudSettings = Boolean(cloud && Object.keys(cloud.settings).length > 0);
  return (
    <section className={`${account.section} ${account.stack}`}>
      <h2 className={account.heading}>设置同步</h2>
      <p className={account.note}>
        同步输入方案、繁体输出、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。
      </p>
      {cloud && <p className={account.muted}>云端版本：{cloud.revision}</p>}
      <div className={account.actionRow}>
        <ActionButton
          action={() => void load()}
          ariaBusy={busy}
          className="secondary"
          disabled={busy}
          label="刷新云端设置"
        />
        <ActionButton
          action={() => setConfirmation("upload")}
          ariaBusy={busy}
          className={account.primary}
          disabled={busy || !cloud || !schema}
          label="上传本机设置"
        />
        <ActionButton
          action={() => setConfirmation("apply")}
          ariaBusy={busy}
          className="secondary"
          disabled={busy || !cloud || !schema || !hasCloudSettings}
          label="下载并应用云端设置"
        />
      </div>
      {busy && <p role="status">正在处理…</p>}
      {message && <p role="status">{message}</p>}
      {confirmation && (
        <div
          className={account.confirmation}
          role="alertdialog"
          aria-label={confirmation === "upload" ? "确认上传本机设置" : "确认应用云端设置"}
        >
          <p className={account.note}>
            {confirmation === "upload"
              ? "将更新云端对应设置，并保留其他平台专属设置。版本冲突时不会自动覆盖。"
              : "将替换本机对应设置，不会下载词库或开启数据上传。"}
          </p>
          <div className={account.actionRow}>
            <ActionButton
              action={() => void runConfirmed()}
              ariaBusy={busy}
              className={account.primary}
              disabled={busy}
              label={confirmation === "upload" ? "确认上传" : "确认应用"}
            />
            <ActionButton
              action={() => setConfirmation(null)}
              ariaBusy={busy}
              className="secondary"
              disabled={busy}
              label="取消"
            />
          </div>
        </div>
      )}
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
  onOpenCloudDictionary,
  onOpenCloudClipboard,
  onOpenAbout,
  onOpenFeedback,
  onOpenDesktopDownload,
  onReplayOnboarding,
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
  onOpenCloudDictionary?: () => void;
  onOpenCloudClipboard?: () => void;
  onOpenAbout?: () => void;
  /** Opens 反馈 (with 使用帮助 under it). A touch host passes it: 我的 is where the design keeps 帮助与反馈. */
  onOpenFeedback?: () => void;
  onOpenDesktopDownload?: () => void;
  onReplayOnboarding?: () => void;
}) {
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
      onOpenCloudDictionary={onOpenCloudDictionary}
      onOpenCloudClipboard={onOpenCloudClipboard}
      onOpenAbout={onOpenAbout}
      onOpenFeedback={onOpenFeedback}
      onOpenDesktopDownload={onOpenDesktopDownload}
      onReplayOnboarding={onReplayOnboarding}
    />
  );
}

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
  onOpenCloudDictionary,
  onOpenCloudClipboard,
  onOpenAbout,
  onOpenFeedback,
  onOpenDesktopDownload,
  onReplayOnboarding,
}: {
  client: AccountClient;
  appIcon?: AppIconClient;
  platform?: "android" | "ios" | "harmony";
  mobile?: boolean;
  onCancelLogin?: () => void;
  onLoginComplete?: () => void;
  onOpenPublishedSkins?: () => void;
  onOpenLocalDesigns?: () => void;
  onOpenCommunity?: (destination: AccountCommunityDestination) => void;
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
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [providers, setProviders] = useState<AccountProviders>({ email: false, phone: false });
  const [user, setUser] = useState<AccountUser | null>(null);
  const [profile, setProfile] = useState<AccountProfile | null>(null);
  const [channel, setChannel] = useState<Channel | null>(null);
  const [target, setTarget] = useState("");
  const [code, setCode] = useState("");
  const [challenge, setChallenge] = useState<AccountChallenge | null>(null);
  const [expiresAt, setExpiresAt] = useState(0);
  const [resendAt, setResendAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());
  const [name, setName] = useState("");
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [editingProfile, setEditingProfile] = useState(false);
  const [mobileProfilePage, setMobileProfilePage] = useState(false);
  const [copiedAccountId, setCopiedAccountId] = useState(false);
  const [googleWaiting, setGoogleWaiting] = useState(false);
  const googleWaitingRef = useRef(false);
  const mounted = useRef(true);
  const clientGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    actionRunning.current = false;
    googleWaitingRef.current = false;
    setGoogleWaiting(false);
    setBusy(false);
    return () => {
      mounted.current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client]);

  const cancelGoogle = () => {
    if (!googleWaitingRef.current || !client.googleCancel) return;
    // The pending googleLogin reports the outcome; a failed cancel only means there was nothing left to cancel.
    void client.googleCancel().catch(() => undefined);
  };

  // Leaving the page must not leave the loopback listener waiting for a browser the user abandoned.
  useEffect(() => () => cancelGoogle(), [client]);

  useEffect(() => {
    if (!mobile || typeof window === "undefined") return;
    const onPopState = (event: PopStateEvent) => {
      setMobileProfilePage(
        event.state?.msimeSettings === true && event.state.accountSubpage === "profile",
      );
    };
    setMobileProfilePage(window.history.state?.accountSubpage === "profile");
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [mobile]);

  const applyProfile = (value: AccountProfile) => {
    setProfile(value);
    setUser(value.user);
    setName(value.user.displayName);
  };

  const loadProfile = async (generation = clientGeneration.current) => {
    const value = await client.profile();
    if (mounted.current && generation === clientGeneration.current) applyProfile(value);
  };

  useEffect(() => {
    let active = true;
    void Promise.allSettled([client.status(), client.providers()]).then(async (results) => {
      if (!active) return;
      const [status, available] = results;
      if (available.status === "fulfilled") setProviders(available.value);
      else if (!isAccountCancellation(available.reason)) setError(accountMessage(available.reason));
      if (status.status === "fulfilled") {
        const nextUser = status.value.user ?? null;
        setUser(nextUser);
        if (nextUser) {
          setName(nextUser.displayName);
          try {
            const value = await client.profile();
            if (active) applyProfile(value);
          } catch (profileError) {
            if (active && !isAccountCancellation(profileError))
              setError(accountMessage(profileError));
          }
        }
      } else {
        if (!isAccountCancellation(status.reason)) setError(accountMessage(status.reason));
      }
      if (active) setLoading(false);
    });
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    if (!challenge) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [challenge]);

  const perform = async (operation: () => Promise<void>) => {
    if (actionRunning.current) return;
    const generation = clientGeneration.current;
    actionRunning.current = true;
    try {
      await runAccountOperation(
        {
          busy,
          isCurrent: () => mounted.current && generation === clientGeneration.current,
          setBusy,
          setError,
          setNotice,
        },
        operation,
      );
    } finally {
      if (generation === clientGeneration.current) actionRunning.current = false;
    }
  };

  const chooseChannel = (value: Channel) => {
    cancelGoogle();
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
      if (!challenge || code.length !== 6 || !/^\d{6}$/.test(code) || expiresAt <= Date.now())
        throw { code: "account_invalid" };
      const result = await client.login(challenge.challengeId, code);
      if (!mounted.current || generation !== clientGeneration.current) return;
      if (!result.user) throw { code: "account_unavailable" };
      setUser(result.user);
      setChannel(null);
      setChallenge(null);
      setCode("");
      await loadProfile(generation);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice("登录成功。");
      onLoginComplete?.();
    });

  const signOut = (all: boolean) =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.logout(all);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setConfirmation(null);
      setNotice(all ? "已退出所有设备。" : "已退出登录。");
    });

  const deleteAccount = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.deleteAccount();
      if (!mounted.current || generation !== clientGeneration.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setConfirmation(null);
      setNotice("账号已注销。");
    });

  const clearExpired = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      await client.clearExpired();
      if (!mounted.current || generation !== clientGeneration.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setNotice("已清除失效登录状态。");
    });

  const rename = () =>
    void perform(async () => {
      const generation = clientGeneration.current;
      const normalized = name.trim();
      if (!normalized || [...normalized].length > 64 || /[\u0000-\u001f\u007f]/.test(normalized))
        throw { code: "account_invalid" };
      const updated = await client.rename(normalized);
      if (!mounted.current || generation !== clientGeneration.current) return;
      applyProfile(updated);
      setNotice("昵称已更新。");
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
      setCopied: setCopiedAccountId,
      setError,
    });
  };

  const openPublishedSkins = onOpenCommunity
    ? () => onOpenCommunity("published-skins")
    : onOpenPublishedSkins;

  if (loading)
    return (
      <div className={account.page}>
        <p role="status">正在读取账号状态…</p>
      </div>
    );

  if (mobileProfilePage && user)
    return (
      <MobileAccountProfilePage
        client={client}
        user={user}
        profile={profile}
        onBack={() => {
          if (typeof window !== "undefined") window.history.back();
          else setMobileProfilePage(false);
        }}
        onSignedOut={() => {
          setMobileProfilePage(false);
          setUser(null);
          setProfile(null);
          setName("");
          if (typeof window !== "undefined") window.history.back();
        }}
        onProfileUpdated={applyProfile}
      />
    );

  const resendSeconds = Math.max(0, Math.ceil((resendAt - now) / 1000));
  const expired = Boolean(challenge) && expiresAt <= now;
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
      setUser(result.user);
      await loadProfile(generation);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice("登录成功。");
      onLoginComplete?.();
    });

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
              pushMobileSettingsState({ page: "account", accountSubpage: "profile" });
              setMobileProfilePage(true);
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
                    : "登录后在设备之间同步设置和词库，还可以发布你的候选窗口皮肤"}
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
                    当前没有可用的验证码登录方式，请稍后重试。
                  </p>
                )}
              </div>
              <div className={account.signInFooter}>
                <ActionButton
                  action={() =>
                    void perform(async () => {
                      const generation = clientGeneration.current;
                      const value = await client.providers();
                      if (!mounted.current || generation !== clientGeneration.current) return;
                      setProviders(value);
                    })
                  }
                  className={account.link}
                  disabled={busy}
                  label="刷新登录方式"
                />
                <ActionButton
                  action={clearExpired}
                  className={account.link}
                  disabled={busy}
                  label="清除失效登录状态"
                />
              </div>
            </>
          ) : (
            <div className={account.signInBody}>
              <AccountInputField
                label={channel === "email" ? "邮箱地址" : "手机号（含国家区号）"}
                ariaLabel={channel === "email" ? "邮箱地址" : "手机号（含国家区号）"}
                type={channel === "email" ? "email" : "tel"}
                autoComplete={channel === "email" ? "email" : "tel"}
                maxLength={320}
                value={target}
                disabled={busy}
                onChange={(value) => {
                  setTarget(value);
                  setChallenge(null);
                  setCode("");
                }}
              />
              <ActionButton
                action={requestCode}
                className={challenge ? account.provider : account.submit}
                disabled={busy || !target.trim() || resendSeconds > 0}
                label={resendSeconds > 0 ? `${resendSeconds} 秒后可重新发送` : "获取验证码"}
              />
              {challenge && (
                <div className={account.code}>
                  <AccountInputField
                    label="6 位验证码"
                    ariaLabel="6 位验证码"
                    inputMode="numeric"
                    autoComplete="one-time-code"
                    maxLength={6}
                    value={code}
                    disabled={busy}
                    onChange={(value) => setCode(value.replace(/\D/g, "").slice(0, 6))}
                  />
                  <ActionButton
                    action={signIn}
                    className={account.submit}
                    disabled={busy || expired || !/^\d{6}$/.test(code)}
                    label={expired ? "验证码已过期，请重新获取" : busy ? "正在登录…" : "登录"}
                  />
                </div>
              )}
              <p className={`${account.muted} m-0 text-center`}>
                验证码只用于本次登录，请勿向他人透露。
              </p>
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
            <AccountNicknameField
              value={name}
              disabled={busy}
              ariaLabel="编辑社区昵称"
              onChange={setName}
            />
            <p className={account.muted}>昵称会显示在社区作品中，已发布的作品也会同步更新。</p>
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
                disabled={busy || name.trim() === user.displayName}
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
      {user && !mobile && (onOpenCloudDictionary || onOpenCloudClipboard) && (
        <GroupList title="云端">
          {onOpenCloudDictionary && (
            <MeRow title="云词库" disabled={busy} onClick={onOpenCloudDictionary} />
          )}
          {onOpenCloudClipboard && (
            <MeRow title="云剪贴板" disabled={busy} onClick={onOpenCloudClipboard} />
          )}
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
                onConfirm={() => (confirmation === "delete" ? deleteAccount() : signOut(true))}
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
      {mobile ? (
        <section className={`${account.section} ${account.stack}`}>
          <h2 className={account.heading}>本地数据与云端作品</h2>
          <p className={account.note}>
            皮肤设计和打字统计保存在本机。只有你主动发布的作品会分享至社区；账号登录不会自动上传本地设计或输入记录。
          </p>
        </section>
      ) : (
        <p className={account.footnote}>
          皮肤设计和打字统计保存在本机。只有你主动发布的作品会分享至社区；账号登录不会自动上传本地设计或输入记录。
        </p>
      )}
    </div>
  );
}
