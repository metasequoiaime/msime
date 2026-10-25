import type { KeybindingPreferences } from "./keybinding-preferences";

/** 中英文切换快捷键各占一项；Ctrl+Space 只在 Linux 提供，另有「不使用」。 */
export type LanguageSwitchChoice = "shift" | "ctrl" | "ctrl_space" | "ctrl_alt_space" | "none";

type LanguageSwitchBinding =
  | "switch_language_shift"
  | "switch_language_ctrl"
  | "switch_language_ctrl_space"
  | "switch_language_ctrl_alt_space";

/** 同时开着多个时按选项顺序显示第一项，不在打开页面时改写偏好。 */
const languageSwitchBindings: [Exclude<LanguageSwitchChoice, "none">, LanguageSwitchBinding][] = [
  ["shift", "switch_language_shift"],
  ["ctrl", "switch_language_ctrl"],
  ["ctrl_space", "switch_language_ctrl_space"],
  ["ctrl_alt_space", "switch_language_ctrl_alt_space"],
];

/** Linux 旧配置缺少 Ctrl+Space 字段时仍默认开启；其余平台不读取这一项。 */
export function languageSwitchChoice(
  keybindings: KeybindingPreferences,
  linux: boolean,
): LanguageSwitchChoice {
  return (
    languageSwitchBindings.find(([, binding]) =>
      binding === "switch_language_ctrl_space"
        ? linux && (keybindings[binding] ?? true)
        : keybindings[binding],
    )?.[0] ?? "none"
  );
}

/** 用户选择时一次更新当前平台支持的所有切换键；「不使用」全部关闭。 */
export function languageSwitchPatch(
  choice: LanguageSwitchChoice,
  linux: boolean,
): Pick<KeybindingPreferences, LanguageSwitchBinding> {
  return {
    switch_language_shift: choice === "shift",
    switch_language_ctrl: choice === "ctrl",
    ...(linux ? { switch_language_ctrl_space: choice === "ctrl_space" } : {}),
    switch_language_ctrl_alt_space: choice === "ctrl_alt_space",
  };
}

/** 轻点与长按的分界在三个桌面宿主上相同（`shared/input/CapsLockTap.h` 的 300 毫秒）；各平台的限制写在说明里。 */
export function capsLockLanguageSwitchDescription(macos: boolean, linux: boolean): string {
  const base =
    "轻点 Caps Lock（300 毫秒内松开）切换中英文，不打开大写锁定；按住更久照常打开大写锁定，大写锁定开着时按一下照常关掉。";
  if (macos)
    return `${base}若在系统设置 › 键盘 › 文字输入中打开了「使用大写锁定键切换 ABC」，系统会先截走轻点，请把它关掉。部分外接键盘不报告 Caps Lock 是否按住，第一次在这类键盘上轻点时，系统会请求「输入监控」授权：允许后轻点才能切换，不允许时轻点照常打开大写锁定。`;
  if (linux)
    return `${base}只在 X11 会话中可用；Wayland 会话里大写锁定由桌面环境掌管，轻点照常打开大写锁定。`;
  return base;
}

export function languageSwitchOptions(
  macos: boolean,
  linux: boolean,
): readonly (readonly [LanguageSwitchChoice, string])[] {
  // 按宿主显示按键名称：macOS 使用 Control 和 Option。
  const options: [LanguageSwitchChoice, string][] = [
    ["shift", "Shift"],
    ["ctrl", macos ? "单击 Control" : "单击 Ctrl"],
    ["ctrl_alt_space", macos ? "Control+Option+Space" : "Ctrl+Alt+Space"],
    ["none", "不使用"],
  ];
  if (linux) options.splice(2, 0, ["ctrl_space", "Ctrl+Space"]);
  return options;
}

/** HarmonyOS 上 `KeyboardSession` 路由的语言切换组合键（Shift、单独的 Ctrl、Ctrl+Alt+Space），使用设计稿的标签。 */
export const harmonyLanguageSwitchOptions: readonly (readonly [LanguageSwitchChoice, string])[] = [
  ["shift", "Shift"],
  ["ctrl", "Ctrl"],
  ["ctrl_alt_space", "Ctrl + Alt + Space"],
  ["none", "无"],
];

type CharacterSetChoice = "ctrl_shift_f" | "none";

export const harmonyCharacterSetOptions: readonly (readonly [CharacterSetChoice, string])[] = [
  ["ctrl_shift_f", "Ctrl + Shift + F"],
  ["none", "无"],
];
