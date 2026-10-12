import { logo, settingsIcons } from "./app-resources";

/** 共享设置外壳里的设置页，按导航顺序排列。 */
export const pages = [
  { id: "home", title: "首页", icon: logo },
  { id: "input", title: "输入", icon: settingsIcons.input },
  {
    id: "expression",
    title: "标点与翻译",
    icon: settingsIcons.expression,
  },
  {
    id: "shortcuts",
    title: "快捷键",
    icon: settingsIcons.shortcuts,
  },
  {
    id: "dictionary",
    title: "词库",
    icon: settingsIcons.dictionary,
  },
  { id: "skin", title: "主题", icon: settingsIcons.skin },
  {
    id: "appearance",
    title: "候选窗口",
    icon: settingsIcons.appearance,
  },
  {
    id: "floating-toolbar",
    title: "悬浮工具栏",
    icon: settingsIcons.floatingToolbar,
  },
  {
    id: "screen-keyboard",
    title: "屏幕键盘",
    icon: settingsIcons.screenKeyboard,
  },
  {
    id: "voice",
    title: "语音输入",
    icon: settingsIcons.voice,
  },
  {
    id: "handwriting",
    title: "手写输入",
    icon: settingsIcons.handwriting,
  },
  {
    id: "tools",
    title: "剪贴板",
    icon: settingsIcons.utilities,
  },
  {
    id: "typing-statistics",
    title: "打字统计",
    icon: settingsIcons.statistics,
  },
  {
    id: "plugins",
    title: "插件",
    icon: settingsIcons.plugins,
  },
  { id: "ai", title: "AI 辅助", icon: settingsIcons.ai },
  {
    id: "account",
    title: "账号与同步",
    icon: settingsIcons.account,
  },
  // 「社区」页不属于任何导航分组，桌面宿主的侧栏没有这一页：候选窗口皮肤在「主题」页浏览，插件包在「插件」页浏览，Windows 只从账号页「我的内容」里的词库、回复模板几行进入这一页。触屏宿主没有这一页，社区内容列在皮肤、词库、AI 辅助这些功能页里。
  {
    id: "community",
    title: "社区",
    icon: settingsIcons.community,
  },
  {
    id: "developer",
    title: "维护与诊断",
    icon: settingsIcons.developer,
  },
  {
    id: "feedback",
    title: "帮助与反馈",
    icon: settingsIcons.feedback,
  },
  { id: "about", title: "关于", icon: settingsIcons.about },
  // 不在导航分组里。桌面从「AI 辅助」页里的一行进入（见 `subPageParents`）；触屏宿主上它是底部的「对话」标签页（见 `mobilePrimaryPageIds`），在那里没有上级页面。
  { id: "chat", title: "AI 对话", icon: settingsIcons.help },
  {
    id: "vocabulary",
    title: "背单词",
    icon: settingsIcons.vocabulary,
  },
  { id: "help", title: "帮助", icon: settingsIcons.help },
  // 「关于」里「匿名使用统计」开关下面那一行打开：上报发送什么、包含什么。
  {
    id: "usage-reporting",
    title: "发送哪些内容",
    icon: settingsIcons.about,
  },
  // 触屏宿主「我的 › 隐私」打开：数据怎么处理的几句话、匿名使用统计开关和完整隐私政策的链接。桌面的这些在「关于」里。
  {
    id: "privacy",
    title: "隐私",
    icon: settingsIcons.about,
  },
  // 触屏宿主「设置」根页状态卡片上的「试用键盘」打开的子页面；桌面首页有自己的键盘入口，没有这一页。
  {
    id: "try-keyboard",
    title: "试用键盘",
    icon: settingsIcons.screenKeyboard,
  },
  // Mobile only, and the one page that is a list of the other pages. The phone bar carries the source's four tabs, so everything else is reached the way the source reaches it: through the 设置 tab, down one level, into a list.
  {
    id: "more",
    title: "全部设置",
    icon: settingsIcons.utilities,
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
  { title: "工具", ids: ["tools", "typing-statistics", "plugins", "ai"] },
  { title: "账号", ids: ["account"] },
  { title: "支持", ids: ["developer", "feedback", "about"] },
] as const satisfies readonly { title: string; ids: readonly SettingsPageId[] }[];

/** Pages reached from inside another settings page. */
export const subPageParents: Partial<Record<SettingsPageId, SettingsPageId>> = {
  chat: "ai",
  vocabulary: "dictionary",
  help: "feedback",
  "usage-reporting": "about",
  privacy: "account",
  "try-keyboard": "home",
};
