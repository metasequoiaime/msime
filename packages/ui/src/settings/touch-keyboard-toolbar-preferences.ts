export type TouchToolbarPreferences = {
  layout: boolean;
  emoji: boolean;
  skin: boolean;
  clipboard: boolean;
  ai: boolean;
  character_set: boolean;
  fullwidth: boolean;
  punctuation: boolean;
};

const defaultTouchToolbar: TouchToolbarPreferences = {
  layout: true,
  emoji: true,
  skin: true,
  clipboard: false,
  ai: false,
  character_set: false,
  fullwidth: false,
  punctuation: false,
};

/** 按触屏键盘工具栏的按钮顺序排列，位于语音入口之后。 */
export const touchToolbarOptions: readonly [keyof TouchToolbarPreferences, string][] = [
  ["layout", "键盘设置"],
  ["emoji", "表情"],
  ["skin", "切换皮肤"],
  ["clipboard", "剪贴板历史"],
  ["ai", "AI 润色"],
  ["character_set", "简繁切换"],
  ["fullwidth", "全角 / 半角"],
  ["punctuation", "中英文标点"],
];

/** 宿主键盘顶部语音按钮的行为：多数宿主开始语音输入，iOS 打开应用中最近的识别结果，macOS 和 HarmonyOS 不绘制该入口。 */
export type TouchVoiceShortcutKind = "start-voice" | "last-result" | "hidden";

/** 只合并显示所需的缺省按钮，不修改原偏好。 */
export function touchToolbarPreferences(
  toolbar?: Partial<TouchToolbarPreferences>,
): TouchToolbarPreferences {
  return { ...defaultTouchToolbar, ...toolbar };
}
