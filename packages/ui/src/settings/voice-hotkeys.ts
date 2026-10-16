export type VoiceHotkeyPlatform = "linux" | "macos" | "windows" | "harmony" | "other";
export type VoiceHotkeyKey =
  | "hotkey_ctrl_f9"
  | "hotkey_ralt"
  | "hotkey_rctrl_ralt"
  | "hotkey_ctrl_win"
  | "hotkey_hold_space_lock";

/** 三个按住录音的组合键，按设置页列出的先后。同一时刻宿主只认一个按住的组合键，所以设置页把它们合成一个选择。 */
export const VOICE_HOLD_KEYS = ["hotkey_ralt", "hotkey_rctrl_ralt", "hotkey_ctrl_win"] as const;
export type VoiceHoldKey = (typeof VOICE_HOLD_KEYS)[number];

export function hotkeyOptions(platform: VoiceHotkeyPlatform): [VoiceHotkeyKey, string][] {
  const macos = platform === "macos";
  // HarmonyOS answers an attached keyboard with the Windows hold, lock and toggle semantics (VoiceHotkeyPolicy), so it takes the Windows labels.
  const desktop =
    platform === "macos" ||
    platform === "windows" ||
    platform === "linux" ||
    platform === "harmony";
  return [
    ["hotkey_ctrl_f9", "Ctrl+F9 切换语音"],
    ["hotkey_ralt", macos ? "按住右 Option 录音" : desktop ? "长按右 Alt 录音" : "右 Alt 切换语音"],
    [
      "hotkey_rctrl_ralt",
      macos
        ? "按住右 Control+右 Option 录音"
        : desktop
          ? "长按右 Ctrl+右 Alt 录音"
          : "Ctrl+右 Alt 切换语音",
    ],
    [
      "hotkey_ctrl_win",
      macos ? "按住 Control+Command 录音" : desktop ? "长按 Ctrl+Win 录音" : "Ctrl+Win 切换语音",
    ],
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
  if (platform === "macos")
    return {
      title: "按住录音",
      labels: {
        hotkey_ralt: "右 Option",
        hotkey_rctrl_ralt: "右 Control+右 Option",
        hotkey_ctrl_win: "Control+Command",
      },
    };
  if (platform === "other")
    return {
      title: "切换语音",
      labels: {
        hotkey_ralt: "右 Alt",
        hotkey_rctrl_ralt: "Ctrl+右 Alt",
        hotkey_ctrl_win: "Ctrl+Win",
      },
    };
  return {
    title: "长按录音",
    labels: {
      hotkey_ralt: "右 Alt",
      hotkey_rctrl_ralt: "右 Ctrl+右 Alt",
      hotkey_ctrl_win: "Ctrl+Win",
    },
  };
}

/** 存储里缺某个组合键时宿主用的出厂值，与 client-core 的 `VoiceInputPreferences::default()` 一致：只有右 Alt / 右 Option 默认打开。 */
const HOLD_DEFAULTS: Record<VoiceHoldKey, boolean> = {
  hotkey_ralt: true,
  hotkey_rctrl_ralt: false,
  hotkey_ctrl_win: false,
};

/** 当前选中的组合键：存储里打开的第一个（缺的键按 `HOLD_DEFAULTS` 算）；三个都关时为 null，即「不使用」。以前可以同时打开几个，这样的配置在用户改动之前照旧显示第一个，不改写存储。 */
export function selectedHoldKey(
  values: Partial<Record<VoiceHotkeyKey, boolean>>,
): VoiceHoldKey | null {
  return VOICE_HOLD_KEYS.find((key) => values[key] ?? HOLD_DEFAULTS[key]) ?? null;
}

/** 选中 `key`（null 为「不使用」）时要写入的值：三个一起写，选中的那个为开，其余为关。 */
export function holdKeyPatch(key: VoiceHoldKey | null): Record<VoiceHoldKey, boolean> {
  return {
    hotkey_ralt: key === "hotkey_ralt",
    hotkey_rctrl_ralt: key === "hotkey_rctrl_ralt",
    hotkey_ctrl_win: key === "hotkey_ctrl_win",
  };
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
