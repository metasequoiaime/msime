/** The settings pages in the shared shell's navigation order. */
export const pages = [
  { id: "home", title: "首页", icon: new URL("../assets/msime.svg", import.meta.url).href },
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
    id: "account",
    title: "账户与同步",
    icon: new URL("../assets/account.svg", import.meta.url).href,
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
    id: "community",
    title: "社区",
    icon: new URL("../assets/community.svg", import.meta.url).href,
  },
  {
    id: "download",
    title: "其他平台下载",
    icon: new URL("../assets/download.svg", import.meta.url).href,
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

/** The design's five navigation groups (dc.html `NAV`), by page id. */
export const settingsNavGroups = [
  ["skin", "appearance", "floating-toolbar"],
  ["input", "expression", "shortcuts", "dictionary"],
  ["screen-keyboard", "voice", "handwriting"],
  ["account", "tools", "typing-statistics", "community", "download"],
  ["developer", "feedback", "about"],
] as const satisfies readonly (readonly SettingsPageId[])[];

/** Pages reached from inside another settings page. */
export const subPageParents: Partial<Record<SettingsPageId, SettingsPageId>> = {
  ai: "expression",
  chat: "expression",
  vocabulary: "dictionary",
  help: "feedback",
};

/** Former page ids whose contents now sit on another page. */
export const settingsPageAliases: Record<string, SettingsPageId> = { helpcode: "input" };
