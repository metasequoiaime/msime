// 由共享 Rust 按住录音目录导出；通过 export_voice_hold_key_catalog 更新。
export const VOICE_HOLD_KEYS = ["hotkey_ralt", "hotkey_rctrl_ralt", "hotkey_ctrl_win"] as const;

export const VOICE_HOLD_PRESETS = [
  {
    key: "hotkey_ralt",
    macos_label: "右 Option",
    desktop_label: "右 Alt",
    other_label: "右 Alt",
    macos_action: "按住右 Option 录音",
    desktop_action: "长按右 Alt 录音",
    other_action: "右 Alt 切换语音",
    fallback: true,
  },
  {
    key: "hotkey_rctrl_ralt",
    macos_label: "右 Control+右 Option",
    desktop_label: "右 Ctrl+右 Alt",
    other_label: "Ctrl+右 Alt",
    macos_action: "按住右 Control+右 Option 录音",
    desktop_action: "长按右 Ctrl+右 Alt 录音",
    other_action: "Ctrl+右 Alt 切换语音",
    fallback: false,
  },
  {
    key: "hotkey_ctrl_win",
    macos_label: "Control+Command",
    desktop_label: "Ctrl+Win",
    other_label: "Ctrl+Win",
    macos_action: "按住 Control+Command 录音",
    desktop_action: "长按 Ctrl+Win 录音",
    other_action: "Ctrl+Win 切换语音",
    fallback: false,
  },
] as const;
