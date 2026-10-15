import type { HostCapabilities, FloatingToolbarPreferences } from "../index";

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
  ["english_mode", "英文输入模式", null],
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

export const floatingToolbarScales: readonly FloatingToolbarPreferences["scale_percent"][] = [
  75, 100, 125, 150,
];

export const floatingToolbarFontSizes: readonly FloatingToolbarPreferences["font_size"][] = [
  16, 18, 20, 22, 24, 26, 28,
];

export function floatingToolbarButtonOptions(
  preferences: FloatingToolbarPreferences,
  host?: Pick<HostCapabilities, FloatingToolbarCapability>,
) {
  return [
    { value: "mode_switch" as const, label: "中英文切换", checked: true, disabled: true },
    ...floatingToolbarOptions
      .filter(([, , capability]) => !capability || !host || host[capability])
      .map(([value, label]) => ({ value, label, checked: preferences[value] })),
  ];
}
