import type { FluentIconName } from "../core/fluent-icons";

/** 「首页」标签画房子，不画页面注册表里的应用标志。 */
import { homeIcon } from "./app-resources";

export function mobileTabIcon(id: string, icon: string): string {
  if (id === "home") return homeIcon;
  return icon;
}

/** HarmonyOS 手机为每个底部标签页绘制的 Fluent 图标。 */
export function harmonyTabIcon(id: string): FluentIconName | undefined {
  if (id === "home") return "home";
  // 地球与首页、统计、我的三个字形一样宽；people_community 三人并排明显更宽，仍留给「我的」里「我发布的皮肤」那一行。
  if (id === "community") return "globe";
  if (id === "typing-statistics") return "data_bar_vertical";
  if (id === "account") return "person";
  return undefined;
}

const mobilePrimaryTitles: Record<string, string> = {
  home: "首页",
  community: "社区",
  "typing-statistics": "统计",
  account: "我的",
};

/** 「首页」标签的根页以应用名作大标题：页面内容是状态卡片和各个设置入口，标签名「首页」放在标题上说明不了什么，与国内 App 首页顶部放品牌名的习惯一致。 */
const MOBILE_HOME_PAGE_TITLE = "水杉输入法";

/** What a mobile tab is called, which is not always what its page is called. */
export function mobileTabTitle(id: string, title: string): string {
  if (mobilePrimaryTitles[id]) return mobilePrimaryTitles[id];
  return title;
}

/** 桌面标题不适合移动端界面时，触摸宿主对页面的称呼。「我的」中打开「反馈」的那一行保留自己的「帮助与反馈」标签；这里是页面的标题。 */
export function mobilePageTitle(id: string, title: string): string {
  if (id === "home") return MOBILE_HOME_PAGE_TITLE;
  if (mobilePrimaryTitles[id]) return mobilePrimaryTitles[id];
  if (id === "skin") return "皮肤";
  if (id === "expression") return "表达";
  if (id === "appearance") return "候选栏";
  if (id === "screen-keyboard") return "键盘";
  if (id === "developer") return "开发者选项";
  if (id === "mcp") return "连接 AI 助手";
  if (id === "feedback") return "反馈";
  return title;
}

/** 触屏宿主把这个导航组称为「键盘、语音与手写」，因为手机设置页没有其他方式说明输入入口。 */
export function mobileGroupTitle(title: string): string {
  if (title === "输入方式" || title === "更多输入方式") return "键盘、语音与手写";
  return title;
}
