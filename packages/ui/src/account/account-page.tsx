import { useEffect, useRef, useState } from "react";
import * as account from "./account-style";
import { accountProviderName, preferredAccountName } from "./account-labels";
import { accountMessage, isAccountCancellation } from "./account-errors";

export type AccountUser = {
  id: string;
  displayName: string;
  createdAt: string;
};

export type AccountProviders = {
  email: boolean;
  phone: boolean;
  apple?: boolean;
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
  profile(): Promise<AccountProfile>;
  rename(displayName: string): Promise<AccountProfile>;
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
  const normalizedName = name.trim();
  const validName =
    Boolean(normalizedName) &&
    [...normalizedName].length <= 64 &&
    !/[\u0000-\u001f\u007f]/.test(normalizedName);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    setBusy(false);
    return () => {
      mounted.current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client]);

  const perform = async (operation: () => Promise<void>) => {
    if (busy || !mounted.current) return;
    const generation = clientGeneration.current;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await operation();
    } catch (cause) {
      if (
        mounted.current &&
        generation === clientGeneration.current &&
        !isAccountCancellation(cause)
      )
        setError(accountMessage(cause));
    } finally {
      if (mounted.current && generation === clientGeneration.current) setBusy(false);
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
    if (!navigator.clipboard?.writeText) return;
    void navigator.clipboard
      .writeText(user.id)
      .then(() => {
        if (!mounted.current) return;
        setCopied(true);
        window.setTimeout(() => {
          if (mounted.current) setCopied(false);
        }, 1800);
      })
      .catch(() => {
        if (mounted.current) setError("账号 ID 暂时无法复制，请稍后重试。");
      });
  };

  return (
    <div className={account.page}>
      <div className={account.profilePageHeader}>
        <button type="button" className="secondary" disabled={busy} onClick={onBack}>
          ‹ 返回
        </button>
        <h2 className={account.heading}>编辑资料</h2>
      </div>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
        </p>
      )}
      <section className={`${account.section} ${account.profilePreviewLarge}`}>
        <div className={account.avatar("large")} aria-hidden="true">
          {(normalizedName || "水杉用户").slice(0, 1)}
        </div>
        <h2 className={account.heading}>{normalizedName || "你的昵称"}</h2>
        <p className={account.muted}>在水杉，留下你的名字</p>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>社区昵称</h2>
        <label className={account.field}>
          社区昵称
          <input
            className={account.input}
            aria-label="编辑社区昵称"
            maxLength={64}
            value={name}
            disabled={busy}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <p className={`account-muted${!validName && normalizedName ? " error" : ""}`}>
          {normalizedName
            ? validName
              ? "昵称会显示在社区作品中，已发布的作品也会同步更新。"
              : "昵称最多 64 个字符，请勿使用换行或控制字符。"
            : "取一个喜欢的名字，让大家记住你。"}
        </p>
        <p className={account.muted}>{[...normalizedName].length}/64</p>
        <div className={account.actionRow}>
          <button
            type="button"
            className={account.primary}
            disabled={busy || !validName || normalizedName === user.displayName}
            onClick={rename}
          >
            保存昵称
          </button>
        </div>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>账号信息</h2>
        <dl className={account.details}>
          <div>
            <dt>账号 ID</dt>
            <dd>
              <button type="button" className={account.copyId} onClick={copyId}>
                {copied ? "已复制" : `#${user.id.slice(0, 6).toUpperCase()}`}
              </button>
            </dd>
          </div>
          <div>
            <dt>登录方式</dt>
            <dd>{profile?.providers.map(accountProviderName).join("、") || "正在读取"}</dd>
          </div>
          <div>
            <dt>加入水杉</dt>
            <dd>{new Date(user.createdAt).toLocaleDateString("zh-CN")}</dd>
          </div>
        </dl>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>账号操作</h2>
        <div className={account.actionRow}>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => setConfirmation("logout")}
          >
            退出登录
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => setConfirmation("logout-all")}
          >
            退出所有设备
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => setConfirmation("relogin")}
          >
            重新登录
          </button>
          <button
            type="button"
            className="danger-text"
            disabled={busy}
            onClick={() => setConfirmation("delete")}
          >
            注销账号
          </button>
        </div>
      </section>
      {confirmation && (
        <div
          className={account.confirmation}
          role="alertdialog"
          aria-label={
            confirmation === "delete"
              ? "确认注销账号"
              : confirmation === "logout-all"
                ? "确认退出所有设备"
                : confirmation === "relogin"
                  ? "确认重新登录"
                  : "确认退出登录"
          }
        >
          <p className={account.note}>
            {confirmation === "delete"
              ? "注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。"
              : confirmation === "logout-all"
                ? "退出所有设备后，所有设备都需要重新登录。"
                : confirmation === "relogin"
                  ? "清除本机登录状态后需要重新登录。"
                  : "退出登录后，社区功能需要重新登录才能使用。"}
          </p>
          <div>
            <button
              type="button"
              className={confirmation === "delete" ? "danger-text" : "account-primary"}
              disabled={busy}
              onClick={confirmAction}
            >
              确认
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy}
              onClick={() => setConfirmation(null)}
            >
              取消
            </button>
          </div>
        </div>
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

  useEffect(() => {
    let active = true;
    const current = ++generation.current;
    mounted.current = true;
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
    if (!info?.supported || pending || info.selected === style) return;
    const current = generation.current;
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
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {info && !info.supported && <p className={account.muted}>当前设备暂不支持更换 App 图标。</p>}
      {info && (
        <div className={account.iconGrid}>
          {appIconOptions.map((option) => {
            const selected = info.selected === option.id;
            const changing = pending === option.id;
            return (
              <button
                type="button"
                className={account.iconCard(selected)}
                key={option.id}
                disabled={!info.supported || pending !== null}
                aria-label={`${option.title}，${option.detail}`}
                aria-pressed={selected}
                onClick={() => void choose(option.id)}
              >
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
              </button>
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

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const load = async () => {
    if (busy || !mounted.current) return;
    const current = generation.current;
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
      if (mounted.current && generation.current === current) setBusy(false);
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
    };
  }, [client, userId]);

  const runConfirmed = async () => {
    if (!cloud || !schema || !confirmation || busy) return;
    const current = generation.current;
    setBusy(true);
    setMessage("");
    const operation = confirmation;
    setConfirmation(null);
    try {
      if (operation === "upload") {
        const next = await client.upload();
        if (!mounted.current || generation.current !== current) return;
        setCloud(next);
      } else {
        await client.apply(userId, cloud);
        if (!mounted.current || generation.current !== current) return;
      }
      if (mounted.current && generation.current === current)
        setMessage(
          operation === "upload"
            ? "本机设置已上传。"
            : "已应用云端设置。请重新打开键盘使部分设置生效。",
        );
    } catch (error) {
      if (mounted.current && generation.current === current && !isAccountCancellation(error))
        setMessage(accountMessage(error));
    } finally {
      if (mounted.current && generation.current === current) setBusy(false);
    }
  };

  const hasCloudSettings = Boolean(cloud && Object.keys(cloud.settings).length > 0);
  return (
    <section className={`${account.section} ${account.stack}`}>
      <h2 className={account.heading}>设置同步</h2>
      <p className={account.note}>
        同步输入方案、简繁体、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。
      </p>
      {cloud && <p className={account.muted}>云端版本：{cloud.revision}</p>}
      <div className={account.actionRow}>
        <button type="button" className="secondary" disabled={busy} onClick={() => void load()}>
          刷新云端设置
        </button>
        <button
          type="button"
          className={account.primary}
          disabled={busy || !cloud || !schema}
          onClick={() => setConfirmation("upload")}
        >
          上传本机设置
        </button>
        <button
          type="button"
          className="secondary"
          disabled={busy || !cloud || !schema || !hasCloudSettings}
          onClick={() => setConfirmation("apply")}
        >
          下载并应用云端设置
        </button>
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
          <div>
            <button
              type="button"
              className={account.primary}
              disabled={busy}
              onClick={() => void runConfirmed()}
            >
              {confirmation === "upload" ? "确认上传" : "确认应用"}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy}
              onClick={() => setConfirmation(null)}
            >
              取消
            </button>
          </div>
        </div>
      )}
    </section>
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
  const mounted = useRef(true);
  const clientGeneration = useRef(0);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    setBusy(false);
    return () => {
      mounted.current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client]);

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
    if (busy || !mounted.current) return;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await operation();
    } catch (operationError) {
      if (mounted.current && !isAccountCancellation(operationError))
        setError(accountMessage(operationError));
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  const chooseChannel = (value: Channel) => {
    setChannel(value);
    setTarget("");
    setCode("");
    setChallenge(null);
    setError("");
    setNotice("");
  };

  const requestCode = () =>
    void perform(async () => {
      if (!channel) return;
      const normalized = target.trim();
      if (!normalized) throw { code: "account_invalid" };
      const value = await client.requestCode(channel, normalized);
      if (!mounted.current) return;
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
      await client.logout(all);
      if (!mounted.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setConfirmation(null);
      setNotice(all ? "已退出所有设备。" : "已退出登录。");
    });

  const deleteAccount = () =>
    void perform(async () => {
      await client.deleteAccount();
      if (!mounted.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setConfirmation(null);
      setNotice("账号已注销。");
    });

  const clearExpired = () =>
    void perform(async () => {
      await client.clearExpired();
      if (!mounted.current) return;
      setUser(null);
      setProfile(null);
      setName("");
      setNotice("已清除失效登录状态。");
    });

  const rename = () =>
    void perform(async () => {
      const normalized = name.trim();
      if (!normalized || [...normalized].length > 64 || /[\u0000-\u001f\u007f]/.test(normalized))
        throw { code: "account_invalid" };
      const updated = await client.rename(normalized);
      if (!mounted.current) return;
      applyProfile(updated);
      setNotice("昵称已更新。");
    });

  const copyAccountId = () => {
    if (!user || !navigator.clipboard?.writeText) return;
    void navigator.clipboard
      .writeText(user.id)
      .then(() => {
        if (!mounted.current) return;
        setCopiedAccountId(true);
        window.setTimeout(() => {
          if (mounted.current) setCopiedAccountId(false);
        }, 1800);
      })
      .catch(() => {
        if (mounted.current) setError("账号 ID 暂时无法复制，请稍后重试。");
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
  const enabledProviders =
    Number(providers.email) + Number(providers.phone) + Number(providers.apple === true);

  const signInWithApple = () =>
    void perform(async () => {
      if (!client.appleLogin) throw { code: "account_unavailable" };
      const result = await client.appleLogin();
      if (!result.user) throw { code: "account_unavailable" };
      setUser(result.user);
      await loadProfile();
      setNotice("登录成功。");
      onLoginComplete?.();
    });

  return (
    <div className={account.page}>
      {!user && onCancelLogin && (
        <div className={account.profilePageHeader}>
          <button type="button" className="secondary" disabled={busy} onClick={onCancelLogin}>
            取消
          </button>
          <h2 className={account.heading}>登录水杉</h2>
        </div>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
        </p>
      )}
      <button
        type="button"
        className={`${account.section} ${account.hero} ${account.profileCard}`}
        disabled={!user || busy}
        aria-label={user ? "编辑个人资料" : undefined}
        onClick={() => {
          if (!user) return;
          if (mobile && typeof window !== "undefined") {
            const current = window.history.state;
            window.history.pushState(
              {
                ...(current && typeof current === "object" ? current : {}),
                msimeSettings: true,
                page: "account",
                accountSubpage: "profile",
              },
              "",
            );
            setMobileProfilePage(true);
          } else setEditingProfile(true);
        }}
      >
        <div className={account.avatar("medium")} aria-hidden="true">
          {user ? preferredAccountName(user).slice(0, 1) : "杉"}
        </div>
        <div>
          <h2 className={account.heading}>{user ? preferredAccountName(user) : "欢迎来到水杉"}</h2>
          <p className={account.note}>{user ? "水杉账号已登录" : "登录，分享你的键盘设计"}</p>
        </div>
        {user && (
          <span className={account.profileChevron} aria-hidden="true">
            ›
          </span>
        )}
      </button>
      {appIcon && (
        <AppIconSettingsCard
          client={appIcon}
          platform={platform === "harmony" ? undefined : platform}
        />
      )}
      {onOpenLocalDesigns && (
        <section className={`${account.section} ${account.communityActions}`}>
          <div>
            <h2 className={account.heading}>我的设计</h2>
            <p className={account.note}>保存在本机的键盘皮肤，不会因登录账号而上传。</p>
          </div>
          <button type="button" className="secondary" disabled={busy} onClick={onOpenLocalDesigns}>
            打开设计器
          </button>
        </section>
      )}
      {user ? (
        <>
          {!mobile && (
            <section className={`${account.section} ${account.stack}`}>
              <div>
                <h2 className={account.heading}>个人资料</h2>
                <p className={account.note}>昵称会显示在社区作品中，已发布的作品也会同步更新。</p>
              </div>
              <label className={account.field}>
                社区昵称
                <input
                  className={account.input}
                  aria-label="社区昵称"
                  maxLength={64}
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  disabled={busy}
                />
              </label>
              <div className={account.actionRow}>
                <button
                  type="button"
                  className={account.primary}
                  disabled={busy || name.trim() === user.displayName}
                  onClick={rename}
                >
                  {busy ? "正在处理…" : "保存昵称"}
                </button>
              </div>
              <dl className={account.details}>
                <div>
                  <dt>账号 ID</dt>
                  <dd>#{user.id.slice(0, 6).toUpperCase()}</dd>
                </div>
                <div>
                  <dt>登录方式</dt>
                  <dd>{profile?.providers.map(accountProviderName).join("、") || "正在读取"}</dd>
                </div>
              </dl>
            </section>
          )}
          {!mobile && editingProfile && (
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
                  <button
                    type="button"
                    className="secondary"
                    disabled={busy}
                    onClick={() => setEditingProfile(false)}
                  >
                    关闭
                  </button>
                </div>
                <div className={account.profilePreview}>
                  <div className={account.avatar("small")} aria-hidden="true">
                    {preferredAccountName(user).slice(0, 1)}
                  </div>
                  <strong>{name.trim() || "你的昵称"}</strong>
                </div>
                <label className={account.field}>
                  社区昵称
                  <input
                    className={account.input}
                    aria-label="编辑社区昵称"
                    maxLength={64}
                    value={name}
                    disabled={busy}
                    onChange={(event) => setName(event.target.value)}
                  />
                </label>
                <p className={account.muted}>昵称会显示在社区作品中，已发布的作品也会同步更新。</p>
                <dl className={account.details}>
                  <div>
                    <dt>账号 ID</dt>
                    <dd>
                      <button type="button" className={account.copyId} onClick={copyAccountId}>
                        {copiedAccountId ? "已复制" : `#${user.id.slice(0, 6).toUpperCase()}`}
                      </button>
                    </dd>
                  </div>
                  <div>
                    <dt>登录方式</dt>
                    <dd>{profile?.providers.map(accountProviderName).join("、") || "正在读取"}</dd>
                  </div>
                  <div>
                    <dt>加入水杉</dt>
                    <dd>{new Date(user.createdAt).toLocaleDateString("zh-CN")}</dd>
                  </div>
                </dl>
                <div className={account.actionRow}>
                  <button
                    type="button"
                    className={account.primary}
                    disabled={busy || name.trim() === user.displayName}
                    onClick={() => {
                      rename();
                      setEditingProfile(false);
                    }}
                  >
                    保存修改
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={busy}
                    onClick={() => setEditingProfile(false)}
                  >
                    取消
                  </button>
                </div>
              </section>
            </div>
          )}
          {!mobile && (
            <section className={`${account.section} ${account.stack}`}>
              <h2 className={account.heading}>账号</h2>
              <div>
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => signOut(false)}
                >
                  退出登录
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => setConfirmation("logout-all")}
                >
                  退出所有设备
                </button>
                <button
                  type="button"
                  className="danger-text"
                  disabled={busy}
                  onClick={() => setConfirmation("delete")}
                >
                  注销账号
                </button>
              </div>
              {confirmation && (
                <div
                  className={account.confirmation}
                  role="alertdialog"
                  aria-label={confirmation === "delete" ? "确认注销账号" : "确认退出所有设备"}
                >
                  <p className={account.note}>
                    {confirmation === "delete"
                      ? "注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。"
                      : "退出所有设备后，所有设备都需要重新登录。"}
                  </p>
                  <div>
                    <button
                      type="button"
                      className={confirmation === "delete" ? "danger-text" : "account-primary"}
                      disabled={busy}
                      onClick={() => (confirmation === "delete" ? deleteAccount() : signOut(true))}
                    >
                      {confirmation === "delete" ? "确认注销账号" : "确认退出所有设备"}
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => setConfirmation(null)}
                    >
                      取消
                    </button>
                  </div>
                </div>
              )}
            </section>
          )}
          {client.settingsSync && (
            <SettingsSyncCard client={client.settingsSync} userId={user.id} />
          )}
          {(onOpenCloudDictionary || onOpenCloudClipboard) && (
            <section className={`${account.section} ${account.communityActions}`}>
              <div>
                <h2 className={account.heading}>云端</h2>
                <p className={account.note}>访问账号中的云词库和云剪贴板。</p>
              </div>
              {onOpenCloudDictionary && (
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={onOpenCloudDictionary}
                >
                  云词库
                </button>
              )}
              {onOpenCloudClipboard && (
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={onOpenCloudClipboard}
                >
                  云剪贴板
                </button>
              )}
            </section>
          )}
          {(openPublishedSkins || onOpenCommunity) &&
            (mobile ? (
              <>
                <section className={`${account.section} ${account.communityGroup}`}>
                  <div>
                    <h2 className={account.heading}>我发布的</h2>
                    <p className={account.note}>管理你公开发布的社区作品。</p>
                  </div>
                  {openPublishedSkins && (
                    <button
                      type="button"
                      className="secondary"
                      aria-label="我发布的皮肤"
                      disabled={busy}
                      onClick={openPublishedSkins}
                    >
                      皮肤
                    </button>
                  )}
                  {onOpenCommunity && (
                    <>
                      <button
                        type="button"
                        className="secondary"
                        aria-label="我发布的词库"
                        disabled={busy}
                        onClick={() => onOpenCommunity("published-dictionary")}
                      >
                        词库
                      </button>
                      <button
                        type="button"
                        className="secondary"
                        aria-label="我发布的回复"
                        disabled={busy}
                        onClick={() => onOpenCommunity("published-reply")}
                      >
                        回复
                      </button>
                    </>
                  )}
                </section>
                {onOpenCommunity && (
                  <section className={`${account.section} ${account.communityGroup}`}>
                    <div>
                      <h2 className={account.heading}>我收藏的</h2>
                      <p className={account.note}>管理你收藏的社区资源。</p>
                    </div>
                    <button
                      type="button"
                      className="secondary"
                      aria-label="收藏的词库"
                      disabled={busy}
                      onClick={() => onOpenCommunity("saved-dictionary")}
                    >
                      词库
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      aria-label="收藏的回复"
                      disabled={busy}
                      onClick={() => onOpenCommunity("saved-reply")}
                    >
                      回复
                    </button>
                  </section>
                )}
              </>
            ) : (
              <section className={`${account.section} ${account.communityActions}`}>
                <div>
                  <h2 className={account.heading}>我的社区作品</h2>
                  <p className={account.note}>管理你公开发布或收藏的社区作品。</p>
                </div>
                {openPublishedSkins && (
                  <button
                    type="button"
                    className="secondary"
                    disabled={busy}
                    onClick={openPublishedSkins}
                  >
                    我发布的皮肤
                  </button>
                )}
                {onOpenCommunity && (
                  <>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => onOpenCommunity("published-dictionary")}
                    >
                      我发布的词库
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => onOpenCommunity("published-reply")}
                    >
                      我发布的回复
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => onOpenCommunity("saved-dictionary")}
                    >
                      收藏的词库
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy}
                      onClick={() => onOpenCommunity("saved-reply")}
                    >
                      收藏的回复
                    </button>
                  </>
                )}
              </section>
            ))}
        </>
      ) : (
        <section className={`${account.section} ${account.stack}`}>
          <h2 className={account.heading}>
            {channel === "email" ? "邮箱登录" : channel === "phone" ? "手机号登录" : "登录方式"}
          </h2>
          {!channel ? (
            <>
              <div className={account.actionRow}>
                {providers.apple && client.appleLogin && (
                  <button
                    type="button"
                    className={account.primary}
                    disabled={busy}
                    onClick={signInWithApple}
                  >
                    使用 Apple 登录
                  </button>
                )}
                {providers.email && (
                  <button
                    type="button"
                    className={account.primary}
                    onClick={() => chooseChannel("email")}
                  >
                    邮箱登录
                  </button>
                )}
                {providers.phone && (
                  <button
                    type="button"
                    className={account.primary}
                    onClick={() => chooseChannel("phone")}
                  >
                    手机号登录
                  </button>
                )}
              </div>
              {enabledProviders === 0 && (
                <p className={account.muted}>当前没有可用的验证码登录方式，请稍后重试。</p>
              )}
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => void perform(async () => setProviders(await client.providers()))}
              >
                刷新登录方式
              </button>
              <button type="button" className="secondary" disabled={busy} onClick={clearExpired}>
                清除失效登录状态
              </button>
            </>
          ) : (
            <>
              <label className={account.field}>
                {channel === "email" ? "邮箱地址" : "手机号（含国家区号）"}
                <input
                  className={account.input}
                  aria-label={channel === "email" ? "邮箱地址" : "手机号（含国家区号）"}
                  type={channel === "email" ? "email" : "tel"}
                  autoComplete={channel === "email" ? "email" : "tel"}
                  maxLength={320}
                  value={target}
                  disabled={busy}
                  onChange={(event) => {
                    setTarget(event.target.value);
                    setChallenge(null);
                    setCode("");
                  }}
                />
              </label>
              <div className={account.actionRow}>
                <button
                  type="button"
                  className={account.primary}
                  disabled={busy || !target.trim() || resendSeconds > 0}
                  onClick={requestCode}
                >
                  {resendSeconds > 0 ? `${resendSeconds} 秒后可重新发送` : "获取验证码"}
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => setChannel(null)}
                >
                  取消
                </button>
              </div>
              {challenge && (
                <div className={account.code}>
                  <label className={account.field}>
                    6 位验证码
                    <input
                      className={account.input}
                      aria-label="6 位验证码"
                      inputMode="numeric"
                      autoComplete="one-time-code"
                      maxLength={6}
                      value={code}
                      disabled={busy}
                      onChange={(event) =>
                        setCode(event.target.value.replace(/\D/g, "").slice(0, 6))
                      }
                    />
                  </label>
                  <button
                    type="button"
                    className={account.primary}
                    disabled={busy || expired || !/^\d{6}$/.test(code)}
                    onClick={signIn}
                  >
                    {expired ? "验证码已过期，请重新获取" : busy ? "正在登录…" : "登录"}
                  </button>
                </div>
              )}
              <p className={account.muted}>验证码只用于本次登录，请勿向他人透露。</p>
            </>
          )}
        </section>
      )}
      {(onOpenAbout || onOpenDesktopDownload) && (
        <section className={`${account.section} ${account.communityActions}`}>
          <div>
            <h2 className={account.heading}>关于</h2>
            <p className={account.note}>查看水杉版本信息、开源说明和其他平台下载指南。</p>
          </div>
          <div className={account.actionRow}>
            {onOpenAbout && (
              <button type="button" className="secondary" disabled={busy} onClick={onOpenAbout}>
                关于水杉
              </button>
            )}
            {onOpenDesktopDownload && (
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={onOpenDesktopDownload}
              >
                电脑版下载
              </button>
            )}
          </div>
        </section>
      )}
      {onReplayOnboarding && (
        <section className={`${account.section} ${account.actionRow}`}>
          <button type="button" className="secondary" disabled={busy} onClick={onReplayOnboarding}>
            重新查看新手引导
          </button>
        </section>
      )}
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>本地数据与云端作品</h2>
        <p className={account.note}>
          皮肤设计和打字统计保存在本机。只有你主动发布的作品会分享至社区；账号登录不会自动上传本地设计或输入记录。
        </p>
      </section>
    </div>
  );
}
