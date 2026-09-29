import type { Preferences } from "../index";

// The last column describes the skin on a host whose skin reaches only the
// candidate window, such as the Linux input method menu.
export const skinOptions: [NonNullable<Preferences["candidate_skin"]>, string, string, string][] = [
  ["fluent", "Fluent", "简洁、紧凑的默认候选窗", "简洁、紧凑的默认候选窗"],
  ["wechat", "微信绿", "微信绿候选窗与悬浮工具栏", "微信绿候选窗"],
  ["graphite", "石墨 Graphite", "克制、平直的候选窗与悬浮工具栏", "克制、平直的候选窗"],
  ["willow_green", "杨柳青 Willow green", "柔和圆角与柳绿色整行高亮", "柔和圆角与柳绿色整行高亮"],
];
