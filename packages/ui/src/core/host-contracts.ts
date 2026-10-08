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

export interface ImeSetupClient {
  /** 最近已知的状态；宿主尚不能回答时为 null。 */
  read(): ImeSetupState | null;
  /** 宿主观察到变化时（例如从系统设置返回）以新状态调用；返回取消订阅函数。 */
  subscribe(listener: (state: ImeSetupState) => void): () => void;
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
