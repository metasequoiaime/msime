import type { HostCapabilities } from "../core/host-capabilities";

import type { FloatingToolbarPreferences } from "./floating-toolbar-preferences";

type FloatingToolbarOptionKey = keyof Pick<
  FloatingToolbarPreferences,
  | "english_mode"
  | "input_scheme"
  | "fullwidth"
  | "punctuation"
  | "character_set"
  | "emoji"
  | "handwriting"
  | "screen_keyboard"
  | "voice"
  | "settings"
>;

type FloatingToolbarCapability = keyof Pick<
  HostCapabilities,
  "floating_toolbar_handwriting" | "floating_toolbar_voice" | "floating_toolbar_input_scheme"
>;

const floatingToolbarOptions: readonly [
  FloatingToolbarOptionKey,
  string,
  FloatingToolbarCapability | null,
][] = [
  // 这一项管的是工具栏上的「中/英」按钮能不能看到，不是英文输入模式本身；Windows 原生设置页也叫它「中英文切换」。
  ["english_mode", "中英文切换", null],
  ["input_scheme", "切换输入方案", "floating_toolbar_input_scheme"],
  ["fullwidth", "全角 / 半角", null],
  ["punctuation", "中英文标点", null],
  ["character_set", "简繁切换", null],
  ["emoji", "表情与符号", null],
  ["handwriting", "手写识别板", "floating_toolbar_handwriting"],
  ["screen_keyboard", "屏幕键盘", null],
  ["voice", "语音输入", "floating_toolbar_voice"],
  ["settings", "设置", null],
];

/** 「工具栏大小」的一档：`percent` 是相对标准大小（100% 缩放、24 图标）的大小，选它时同时写入 `scale_percent` 和 `font_size`。 */
export interface FloatingToolbarSize {
  percent: number;
  scale_percent: FloatingToolbarPreferences["scale_percent"];
  font_size: FloatingToolbarPreferences["font_size"];
}

/**
 * 「工具栏大小」的各档。工具栏的按钮和图标按 `font_size × scale_percent` 画（macOS `FloatingToolbarPanel.mm`、Windows `FloatingToolbarWindow` 都如此），两个下拉调的是同一个结果，所以页面只给一个大小。
 *
 * 前四档只改缩放、图标保持默认的 24；175% 用 150% 缩放加 28 的图标（150 × 28 = 175 × 24），和以前两个下拉都拉满时一样大，最大尺寸没有变小。宿主画间距、logo 和分隔线只乘缩放、不乘图标尺寸（macOS `applySizingPreferences:`），所以从 150% 到 175% 只有按钮和图标变大，间距停在 150%，页面文案因此只说按钮和图标。各档都只用每个宿主已经接受的缩放（75–150）和图标尺寸（16–28），不往共享文档里写任何宿主不认得的新值：Windows 宿主和 PreviewConfig 对超出 75–150 的缩放会整份拒绝工具栏设置，macOS 和 Linux 的读取方也只认这四个缩放。
 */
export const floatingToolbarSizes: readonly FloatingToolbarSize[] = [
  { percent: 75, scale_percent: 75, font_size: 24 },
  { percent: 100, scale_percent: 100, font_size: 24 },
  { percent: 125, scale_percent: 125, font_size: 24 },
  { percent: 150, scale_percent: 150, font_size: 24 },
  { percent: 175, scale_percent: 150, font_size: 28 },
];

/** 存下的缩放和图标尺寸对应的那一档；以前在 Windows 原生设置页或旧版本里单独调过、凑不成任何一档时返回 undefined，页面照实显示「自定义」而不改写存储。 */
export function floatingToolbarSize(
  preferences: Pick<FloatingToolbarPreferences, "scale_percent" | "font_size">,
): FloatingToolbarSize | undefined {
  return floatingToolbarSizes.find(
    (size) =>
      size.scale_percent === preferences.scale_percent && size.font_size === preferences.font_size,
  );
}

/** 工具栏上每个可以关掉的按钮，按显示顺序。中/英按钮也能关（每个宿主都读 `english_mode`），所以没有锁定勾选的「始终显示」一项。 */
export function floatingToolbarButtonOptions(
  preferences: FloatingToolbarPreferences,
  host?: Pick<HostCapabilities, FloatingToolbarCapability>,
) {
  return floatingToolbarOptions
    .filter(([, , capability]) => !capability || !host || host[capability])
    .map(([value, label]) => ({ value, label, checked: preferences[value] }));
}
