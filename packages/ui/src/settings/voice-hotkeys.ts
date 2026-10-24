import { VOICE_HOLD_KEYS, VOICE_HOLD_PRESETS } from "./voice-hold-key-catalog";

export { VOICE_HOLD_KEYS } from "./voice-hold-key-catalog";

export type VoiceHotkeyPlatform = "linux" | "macos" | "windows" | "harmony" | "other";
export type VoiceHotkeyKey =
  | "hotkey_ctrl_f9"
  | "hotkey_ralt"
  | "hotkey_rctrl_ralt"
  | "hotkey_ctrl_win"
  | "hotkey_hold_space_lock";

export type VoiceHoldKey = (typeof VOICE_HOLD_KEYS)[number];

export function hotkeyOptions(platform: VoiceHotkeyPlatform): [VoiceHotkeyKey, string][] {
  const macos = platform === "macos";
  // HarmonyOS 外接键盘采用桌面的按住、锁定和切换语义，读取同一份桌面名称。
  const desktop = platform === "windows" || platform === "linux" || platform === "harmony";
  return [
    ["hotkey_ctrl_f9", "Ctrl+F9 切换语音"],
    ...VOICE_HOLD_PRESETS.map((entry): [VoiceHotkeyKey, string] => [
      entry.key,
      macos ? entry.macos_action : desktop ? entry.desktop_action : entry.other_action,
    ]),
    [
      "hotkey_hold_space_lock",
      platform === "windows" || platform === "linux" || platform === "harmony"
        ? "长按录音时按空格锁定"
        : "空格锁定语音",
    ],
  ];
}

/** 合并后的那一行：标题，以及每个组合键在下拉里的名字（不再带「按住」「录音」，标题已经说了）。 */
export function holdChoice(platform: VoiceHotkeyPlatform): {
  title: string;
  labels: Record<VoiceHoldKey, string>;
} {
  const macos = platform === "macos";
  const other = platform === "other";
  return {
    title: macos ? "按住录音" : other ? "切换语音" : "长按录音",
    labels: Object.fromEntries(
      VOICE_HOLD_PRESETS.map((entry) => [
        entry.key,
        macos ? entry.macos_label : other ? entry.other_label : entry.desktop_label,
      ]),
    ) as Record<VoiceHoldKey, string>,
  };
}

/** 当前选中的组合键：按共享目录顺序读取显式值或出厂默认；都关时为 null。读取不改写旧文档。 */
export function selectedHoldKey(
  values: Partial<Record<VoiceHotkeyKey, boolean>>,
): VoiceHoldKey | null {
  return VOICE_HOLD_PRESETS.find((entry) => values[entry.key] ?? entry.fallback)?.key ?? null;
}

/** 选中 `key`（null 为「不使用」）时要写入的值：三个一起写，选中的那个为开，其余为关。 */
export function holdKeyPatch(key: VoiceHoldKey | null): Record<VoiceHoldKey, boolean> {
  return Object.fromEntries(
    VOICE_HOLD_KEYS.map((candidate) => [candidate, candidate === key]),
  ) as Record<VoiceHoldKey, boolean>;
}

export function description(platform: VoiceHotkeyPlatform): string {
  if (platform === "linux")
    return "在当前输入上下文中生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，Escape 取消。Ctrl+F9 按一次开始、再按一次结束，也能结束锁定的录音。没有 provider 时快捷键不会拦截编辑器输入";
  if (platform === "macos")
    return "输入法启用时按住修饰键快捷键录音，松开结束；组合键先按 Control。按住期间按空格锁定，Escape 取消。修饰键快捷键由输入法自身接收，不需要额外授权；Ctrl+F9 在输入法会话之外接收，需要在「系统设置 › 隐私与安全性 › 输入监控」中允许本输入法，否则按下没有任何反应。首次授权后请重新按键。";
  if (platform === "windows")
    return "输入法运行时全局生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，锁定后再按一次快捷键或点 ✓ 结束，Escape 或 ✗ 取消。Ctrl+F9 按一次开始、再按一次结束。";
  if (platform === "harmony")
    return "连接实体键盘后，在输入状态下生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，Escape 取消。Ctrl+F9 按一次开始、再按一次结束，也能结束锁定的录音。";
  return "输入法运行时全局生效，用于开始和结束语音录音";
}
