import { logo, settingsIcons } from "./app-resources";

/** 共享设置外壳里的设置页；导航顺序由 `settingsNavGroups` 定义。 */
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
  // 「社区」页不属于任何导航分组，桌面宿主的侧栏没有这一页：候选窗口皮肤在「主题」页浏览，插件包在「插件」页浏览，桌面上只有账号页「我的内容」里的几行（发布的与收藏的词库、回复模板等）经 `useSettingsDestinationActions` 的 `openCommunity` 进入这一页，词库页的「社区词库」组和 AI 辅助页的回复模板只在触屏宿主渲染。Windows 的设置入口不打开这一页（React 设置页只在直接运行 `MSIME.exe` 时出现）：原生设置窗口在「账号与同步」页里自己画社区词库与回复模板，`community` 路由打开原生的「主题」页（`platforms/windows/settings/SettingsNavigation.h`）。触屏宿主上这一页是底部「社区」标签的根页，顶部分「皮肤 / 词库」两类完整图库；回复模板仍在「AI 辅助」页上打开。
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
  // 不在导航分组里。桌面从「AI 辅助」页里的一行进入（见 `subPageParents`）；触屏宿主上它从「AI 辅助」页作为二级页面进入。
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
  // 只在宿主提供 MCP 状态时可路由；桌面并进「AI」的连接标签，触屏保留独立页。
  {
    id: "mcp",
    title: "AI 助手",
    icon: settingsIcons.developer,
  },
  // 触屏宿主「我的 › 隐私」打开：数据怎么处理的几句话、匿名使用统计开关和完整隐私政策的链接。桌面的这些在「关于」里。
  {
    id: "privacy",
    title: "隐私",
    icon: settingsIcons.about,
  },
  // 触屏宿主「首页」根页状态卡片上的「试用键盘」打开的子页面；桌面首页有自己的键盘入口，没有这一页。
  {
    id: "try-keyboard",
    title: "试用键盘",
    icon: settingsIcons.screenKeyboard,
  },
] as const;

export type SettingsPageId = (typeof pages)[number]["id"];

/**
 * 一级导航的分组：桌面侧栏、手机「首页」列表和旧的分组导出都从这里派生，导航顺序只有这一个来源，`pages` 只登记页面。Windows 原生设置窗口的 `SettingsNavigation.h` 也要与它保持一致：和共享桌面侧栏一样，是这里去掉 `desktopMergedPages` 之后的页面。
 *
 * 先排输入设置和其他输入方式，再排外观、工具、数据与支持；桌面侧栏按组折叠，触屏列表沿用同一顺序。
 */
export const settingsNavGroups = [
  { title: "打字", ids: ["input", "dictionary", "shortcuts", "expression"] },
  { title: "输入方式", ids: ["voice", "screen-keyboard", "handwriting"] },
  { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
  { title: "工具", ids: ["tools", "ai", "mcp", "plugins"] },
  { title: "数据", ids: ["typing-statistics", "account"] },
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

/**
 * 桌面上并进另一页的页面，键是并进去的页，值是承载它的页。它们仍是可路由的 id：宿主菜单、托盘、旧链接和页内跳转打开它时显示承载页，桌面侧栏不再列出它。`settingsNavGroups` 不变，触屏宿主照旧列出这些页；Windows 原生设置窗口（`SettingsNavigation.h`）和共享桌面侧栏一样不列它们，路由别名把它们指向承载页。
 *
 * 「帮助与反馈」没有一项偏好，只有帮助入口、问题报告和反馈渠道，桌面上画在「关于」里。
 */
export const desktopMergedPages: Partial<Record<SettingsPageId, SettingsPageId>> = {
  feedback: "about",
  mcp: "ai",
};

/** 桌面上真正显示的页面：并进别的页的 id 换成承载它的页，其他原样返回。 */
export function desktopPage(id: SettingsPageId): SettingsPageId {
  return desktopMergedPages[id] ?? id;
}
