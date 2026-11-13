/**
 * 共用设置界面与嵌入它的宿主之间的契约，覆盖 HarmonyOS 改版新增的服务：季节应用主题、输入法启用状态、系统栏颜色和应用内反馈。这里只有类型；各宿主通过 `SettingsClient` 提供自己的实现，不提供某项的宿主就没有对应的界面。
 */

/** `msime_client_app_theme_catalog` 列出的应用主题 id：`siji` 跟随季节，其余四个各固定为一季。 */
export type AppThemeId = "siji" | "chunya" | "xiayin" | "qiushan" | "dongxue";

export type AppSeason = "spring" | "summer" | "autumn" | "winter";

/** 与 `msime_client_resolve_app_theme` 的返回值完全一致。每个颜色都是 `#RRGGBB` 或 `#RRGGBBAA`。 */
export interface ResolvedAppTheme {
  id: AppThemeId;
  /** 实际绘制的季节；`siji` 下由宿主的当前月份决定。 */
  season: AppSeason;
  accent: string;
  accent_soft: string;
  /** `accent` 底色上的文字和图标颜色：浅色模式为白色，深色模式为强调色调深后的颜色。 */
  on_accent: string;
  background: string;
  card: string;
  hair: string;
  /** 触屏候选条首选的字色：浅色是同一色相加深到 WCAG AA 的强调色，深色等于 `accent`。共享设置页不画它，只为与返回值一致。 */
  candidate_accent: string;
}

export interface AppThemeCatalogEntry {
  id: AppThemeId;
  title: string;
  /** 固定主题始终绘制的季节；跟随季节的 `siji` 为 `null`。 */
  season: AppSeason | null;
  seasonal: boolean;
}

export interface AppThemeClient {
  /** 已保存的主题 id。 */
  load(): AppThemeId;
  /** 保存主题 id；宿主无法持久化时返回 false。 */
  save(id: AppThemeId): boolean;
  /** 已保存主题在某一外观下的颜色；宿主暂时无法解析时为 null。 */
  resolve(dark: boolean): ResolvedAppTheme | null;
  catalog(): readonly AppThemeCatalogEntry[];
  /** 已保存的主题或季节变化时调用；返回取消订阅函数。 */
  subscribe(listener: () => void): () => void;
}

/** 本输入法是否已在系统中启用、是否为当前输入法；`null` 表示未知。 */
export interface ImeSetupState {
  enabled: boolean | null;
  current: boolean | null;
}

/** 跨平台首次运行引导的事实快照；`null` 是宿主无法可靠查询，不是未完成。 */
export interface OobeState {
  resourcesReady: boolean | null;
  systemEnabled: boolean | null;
  systemCurrent: boolean | null;
  preferencesSaved: boolean | null;
  onboardingSeen: boolean | null;
}

export type OobeStep =
  | "prepare_resources"
  | "enable_system_input_method"
  | "make_default_input_method"
  | "save_preferences";

/** 宿主提供 OOBE 状态时使用的读取和通知接口。 */
export interface OobeClient {
  read(): OobeState | null;
  subscribe(listener: (state: OobeState) => void): () => void;
  refresh?(): void;
  /** 重试宿主的资源准备，桌面宿主可以打开自己的安装或配置页。 */
  prepareResources?(): Promise<void>;
  /** 打开宿主的安装或系统配置入口。 */
  openSetup?(): Promise<void>;
}

export function oobeFlowComplete(state: OobeState): boolean {
  return (
    state.onboardingSeen === true &&
    state.resourcesReady === true &&
    state.preferencesSaved === true
  );
}

export function oobeSystemReady(state: OobeState): boolean {
  return state.systemEnabled === true && state.systemCurrent === true;
}

export function oobeComplete(state: OobeState): boolean {
  return oobeFlowComplete(state) && oobeSystemReady(state);
}

/** 只返回明确失败的第一步；未知值交给宿主显示无法确认，不强行当成失败。 */
export function oobeNextAction(state: OobeState): OobeStep | null {
  if (state.resourcesReady === false) return "prepare_resources";
  if (state.systemEnabled === false) return "enable_system_input_method";
  if (state.systemCurrent === false) return "make_default_input_method";
  if (state.preferencesSaved === false) return "save_preferences";
  return null;
}

export interface ImeSetupClient {
  /** 最近已知的状态；宿主尚不能回答时为 null。 */
  read(): ImeSetupState | null;
  /** 宿主观察到变化时（例如从系统设置返回）以新状态调用；返回取消订阅函数。 */
  subscribe(listener: (state: ImeSetupState) => void): () => void;
  /** 请宿主现在重新读一次，有变化时照常通知订阅者。用于宿主看不到变化的时候：HarmonyOS 上从键盘的 🌐 菜单切换输入法，不会触发宿主监听的 `imeChange`。 */
  refresh?(): void;
}

/**
 * 宿主自己征得的隐私政策授权（目前只有 HarmonyOS：首次启动时由原生弹窗询问）。没有授权时宿主不提供账号、社区、云同步、检查更新这些联网入口，设置页在「隐私」里说明现状并给出重新打开弹窗的入口。
 */
export interface PrivacyConsentClient {
  /** 用户是否已同意当前版本的隐私政策。授权变化时宿主换一个新的 client。 */
  granted: boolean;
  /** 重新打开宿主的授权弹窗，用户在其中同意、拒绝或撤回同意。 */
  review(): Promise<void>;
}

/**
 * 「防止截屏」（目前只有 HarmonyOS 经共享设置页提供，Android 在原生隐私页）：开着时宿主让设置页面和键盘在截屏、录屏里显示为黑色。开关只属于这台设备，宿主自己存，不进偏好文档、不随账号同步。
 *
 * 切换规则由宿主执行：第一次打开直接保存，打开过以后再开、再关都先过系统认证；设备没有可用的系统认证时一律拒绝。被拒绝或没保存时 `save` 以 `{ code }` 拒绝，开关保持原值，`code` 见 `SCREEN_CAPTURE_AUTHENTICATION_UNAVAILABLE` 和 `SCREEN_CAPTURE_AUTHENTICATION_FAILED`。
 */
export interface ScreenCaptureProtectionClient {
  load(): Promise<boolean>;
  /** 切换到 `enabled`，回答保存后的值。 */
  save(enabled: boolean): Promise<boolean>;
}

export interface HostChromeClient {
  /** 给系统状态栏和导航栏着色以配合页面；`dark` 表示要求浅色的栏内容。 */
  setSystemBars(bars: { background: string; navigationBar: string; dark: boolean }): void;
}

export type FeedbackType = "bug" | "suggestion" | "dictionary";

export interface FeedbackSubmission {
  type: FeedbackType;
  text: string;
  /** 用户同意附带的诊断字段；用户拒绝时为 null。 */
  diagnostics: Record<string, string> | null;
}

export interface FeedbackClient {
  /** 宿主接受反馈后 resolve；未能接受时以原因 reject。 */
  submit(report: FeedbackSubmission): Promise<void>;
}
