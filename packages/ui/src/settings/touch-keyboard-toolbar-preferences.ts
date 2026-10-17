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

/** HarmonyOS 手机键盘空闲工具栏的五个工具和显示方式（共享偏好 `harmony_toolbar`）。标志键和收起键固定在两端，不在这里。 */
export type HarmonyToolbarPreferences = {
  emoji: boolean;
  phrase: boolean;
  clipboard: boolean;
  skin: boolean;
  scheme: boolean;
  /** 显示方式选了「隐藏」：空闲时整行不显示，组字时候选栏照常显示。 */
  hidden: boolean;
};

/** 与 client-core 的缺省相同：五个工具全在、不隐藏，就是没有这组设置时的鸿蒙键盘。 */
const defaultHarmonyToolbar: HarmonyToolbarPreferences = {
  emoji: true,
  phrase: true,
  clipboard: true,
  skin: true,
  scheme: true,
  hidden: false,
};

/** 按鸿蒙工具栏上从左到右的顺序排列，夹在标志键和收起键之间。 */
export const harmonyToolbarOptions: readonly [
  Exclude<keyof HarmonyToolbarPreferences, "hidden">,
  string,
][] = [
  ["emoji", "表情"],
  ["phrase", "常用语"],
  ["clipboard", "剪贴板"],
  ["skin", "皮肤"],
  ["scheme", "输入方式"],
];

/** 只合并显示所需的缺省值，不修改原偏好。 */
export function harmonyToolbarPreferences(
  toolbar?: Partial<HarmonyToolbarPreferences>,
): HarmonyToolbarPreferences {
  return { ...defaultHarmonyToolbar, ...toolbar };
}
