/** 共享设置外壳里的设置页，按导航顺序排列。 */
export const pages = [
  { id: "home", title: "首页", icon: new URL("../assets/msime.svg", import.meta.url).href },
  { id: "input", title: "输入", icon: new URL("../assets/input.svg", import.meta.url).href },
  {
    id: "expression",
    title: "表达",
    icon: new URL("../assets/expression.svg", import.meta.url).href,
  },
  {
    id: "shortcuts",
    title: "快捷键",
    icon: new URL("../assets/shortcut.svg", import.meta.url).href,
  },
  {
    id: "dictionary",
    title: "词库",
    icon: new URL("../assets/dictionary.svg", import.meta.url).href,
  },
  { id: "skin", title: "主题", icon: new URL("../assets/skin.svg", import.meta.url).href },
  {
    id: "appearance",
    title: "候选窗口",
    icon: new URL("../assets/appearance.svg", import.meta.url).href,
  },
  {
    id: "floating-toolbar",
    title: "悬浮工具栏",
    icon: new URL("../assets/floating-toolbar.svg", import.meta.url).href,
  },
  {
    id: "screen-keyboard",
    title: "屏幕键盘",
    icon: new URL("../assets/screen-keyboard.svg", import.meta.url).href,
  },
  {
    id: "voice",
    title: "语音输入",
    icon: new URL("../assets/voice-input.svg", import.meta.url).href,
  },
  {
    id: "handwriting",
    title: "手写输入",
    icon: new URL("../assets/handwriting.svg", import.meta.url).href,
  },
  {
    id: "tools",
    title: "云剪贴板",
    icon: new URL("../assets/utilities.svg", import.meta.url).href,
  },
  {
    id: "typing-statistics",
    title: "统计",
    icon: new URL("../assets/statistics.svg", import.meta.url).href,
  },
  {
    id: "plugins",
    title: "插件",
    icon: new URL("../assets/plugins.svg", import.meta.url).href,
  },
  {
    id: "account",
    title: "账户与同步",
    icon: new URL("../assets/account.svg", import.meta.url).href,
  },
  {
    id: "community",
    title: "社区",
    icon: new URL("../assets/community.svg", import.meta.url).href,
  },
  {
    id: "developer",
    title: "开发者选项",
    icon: new URL("../assets/developer.svg", import.meta.url).href,
  },
  { id: "feedback", title: "反馈", icon: new URL("../assets/feedback.svg", import.meta.url).href },
  { id: "about", title: "关于", icon: new URL("../assets/about.svg", import.meta.url).href },
  // Reached from inside a page rather than from the navigation; see `subPageParents`.
  { id: "ai", title: "AI 辅助", icon: new URL("../assets/ai.svg", import.meta.url).href },
  { id: "chat", title: "AI 对话", icon: new URL("../assets/help.svg", import.meta.url).href },
  {
    id: "vocabulary",
    title: "背单词",
    icon: new URL("../assets/vocabulary.svg", import.meta.url).href,
  },
  { id: "help", title: "帮助", icon: new URL("../assets/help.svg", import.meta.url).href },
  // Mobile only, and the one page that is a list of the other pages. The phone bar carries the source's four tabs, so everything else is reached the way the source reaches it: through the 设置 tab, down one level, into a list.
  {
    id: "more",
    title: "全部设置",
    icon: new URL("../assets/utilities.svg", import.meta.url).href,
  },
] as const;

export type SettingsPageId = (typeof pages)[number]["id"];

/**
 * 一级导航的分组：桌面侧栏、手机「全部设置」列表和旧的分组导出都从这里派生，页面顺序只有这一个来源。Windows 原生设置窗口的 `SettingsNavigation.h` 也要与它保持一致。
 *
 * 「打字」排在「外观」之前：打字行为会反复调整，外观通常只调一次。
 */
export const settingsNavGroups = [
  { title: "打字", ids: ["input", "expression", "shortcuts", "dictionary"] },
  { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
  { title: "更多输入方式", ids: ["screen-keyboard", "voice", "handwriting"] },
  { title: "工具", ids: ["tools", "typing-statistics", "plugins"] },
  { title: "账户与社区", ids: ["account", "community"] },
  { title: "支持", ids: ["developer", "feedback", "about"] },
] as const satisfies readonly { title: string; ids: readonly SettingsPageId[] }[];

/** Pages reached from inside another settings page. */
export const subPageParents: Partial<Record<SettingsPageId, SettingsPageId>> = {
  ai: "expression",
  chat: "expression",
  vocabulary: "dictionary",
  help: "feedback",
};

/** 已不再独立成页的旧页面 id，映射到现在承载其内容的页面。宿主仍会发送这些 id。 */
export const settingsPageAliases: Record<string, SettingsPageId> = {
  helpcode: "input",
  download: "about",
};

/**
 * 包对外导出的页面 id。`helpcode` 已不是独立页面，打开它会经 `settingsPageAliases` 落到输入页；过渡期仍保留在类型里，免得仓库外按旧 id 写的 TypeScript 宿主编译失败。
 */
export type ExportedSettingsPageId = SettingsPageId | "helpcode";
