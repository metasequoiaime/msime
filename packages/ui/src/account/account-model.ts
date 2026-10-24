export type AccountUser = {
  id: string;
  displayName: string;
  createdAt: string;
  /** 关联 Google 账号已验证的邮箱。 */
  email?: string;
  /** 头像变更时随之变化；图片本身由 `AccountClient.avatar` 提供，页面不加载远程图片。 */
  avatarUrl?: string;
  /** 头像由用户上传，可以移除；Google 账号头像不属于此类。 */
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
  /** 卡片上说明同步哪些设置。宿主同步的范围不同（Windows 只同步各平台共有的输入设置）时自己给，缺省是移动端的说明。 */
  description?: string;
  /** 应用云端设置成功后的提示。缺省是移动端的说法，要用户重新打开键盘；设置当场生效的宿主（Windows 的 Server 从共享偏好里实时读到）自己给。 */
  appliedMessage?: string;
  schema(): Promise<AccountPreferenceSchema>;
  load(): Promise<AccountPreferences>;
  upload(): Promise<AccountPreferences>;
  apply(userId: string, preferences: AccountPreferences): Promise<void>;
}

/** 一条 QQ 群助手绑定。服务端只存群和成员标识的摘要，所以只有编号和绑定时间。 */
export type AccountQqLink = {
  id: string;
  createdAt: string;
};

/** 「绑定 QQ 群助手」：群友在 QQ 群里 @机器人 发 /bind 拿到 8 位绑定码，在这里输入。 */
export interface QqLinksClient {
  list(): Promise<AccountQqLink[]>;
  redeem(code: string): Promise<AccountQqLink>;
  remove(id: string): Promise<void>;
}

/** 注销以后删除本机匿名账号名下语音样本的结果，形状与 C 接口 `account_delete` 的 `voice_samples` 相同；宿主不做这一步时没有或为 null。 */
export interface AccountVoiceSamplesResult {
  ok: boolean;
  error?: string;
  retry_after_seconds?: number;
}

/** 注销账号的结果。只有 Tauri 桌面（Windows、Linux）和原生 C 接口带 `voice_samples`，其余宿主不返回值。 */
export interface AccountDeletion {
  voice_samples?: AccountVoiceSamplesResult | null;
}

/** 共享 Rust 昵称纯函数的结果，长度上限不由界面推导。 */
export type AccountNamePresentation = {
  normalized: string;
  preferred: string;
  valid: boolean;
  maximum_chars: number;
  maximum_utf16_units: number;
};

export interface AccountClient {
  /** 不创建账号、不读文件、不联网；失败时昵称编辑不可保存。 */
  namePresentation(name: string, id?: string): Promise<AccountNamePresentation>;
  status(): Promise<{ user?: AccountUser | null }>;
  providers(): Promise<AccountProviders>;
  requestCode(provider: "email" | "phone", target: string): Promise<AccountChallenge>;
  login(challengeId: string, code: string): Promise<{ user?: AccountUser | null }>;
  /** iOS 在原生层执行 nonce 与 AuthenticationServices 交换。 */
  appleLogin?: () => Promise<{ user?: AccountUser | null }>;
  /** 桌面宿主在原生层打开 Google 浏览器并处理回环重定向，页面不接收授权码。 */
  googleLogin?: () => Promise<{ user?: AccountUser | null }>;
  /** 终止等待中的 `googleLogin`，该调用随后以取消拒绝。浏览器不能报告 Google 标签页关闭，用户可借此主动取消，无需等待超时。 */
  googleCancel?: () => Promise<void>;
  profile(): Promise<AccountProfile>;
  rename(displayName: string): Promise<AccountProfile>;
  /** 返回已登录用户头像的 `data:` URL，没有头像时返回 null。不读取头像的宿主不提供此方法，页面显示姓名首字符。 */
  avatar?: () => Promise<string | null>;
  /** 打开平台文件对话框选择 PNG 或 JPEG 并上传；用户关闭对话框时返回 null，页面不接收路径。 */
  chooseAvatar?: () => Promise<AccountProfile | null>;
  /** 移除上传的头像；存在 Google 头像时重新显示它。 */
  removeAvatar?: () => Promise<AccountProfile>;
  logout(all: boolean): Promise<void>;
  deleteAccount(): Promise<AccountDeletion | void>;
  clearExpired(): Promise<void>;
  settingsSync?: SettingsSyncClient;
  qqLinks?: QqLinksClient;
  appIcon?: AppIconClient;
}

export type AccountCommunityDestination =
  | "published-skins"
  | "published-dictionary"
  | "saved-dictionary"
  | "published-reply"
  | "saved-reply";
