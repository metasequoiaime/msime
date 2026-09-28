export type VoiceHotkeyPlatform = "linux" | "macos" | "windows" | "other";
export type VoiceHotkeyKey =
  | "hotkey_ctrl_f9"
  | "hotkey_ralt"
  | "hotkey_rctrl_ralt"
  | "hotkey_ctrl_win"
  | "hotkey_hold_space_lock";

export interface VoiceHotkeysSectionProps {
  platform: VoiceHotkeyPlatform;
  values: Partial<Record<VoiceHotkeyKey, boolean>>;
  onChange: (key: VoiceHotkeyKey, enabled: boolean) => void;
}

function hotkeyOptions(platform: VoiceHotkeyPlatform): [VoiceHotkeyKey, string][] {
  const macos = platform === "macos";
  const desktop = platform === "macos" || platform === "windows" || platform === "linux";
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
      platform === "windows" || platform === "linux" ? "长按录音时按空格锁定" : "空格锁定语音",
    ],
  ];
}

function description(platform: VoiceHotkeyPlatform): string {
  if (platform === "linux")
    return "在当前输入上下文中生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，Escape 取消。Ctrl+F9 按一次开始、再按一次结束，也能结束锁定的录音。没有 provider 时快捷键不会拦截编辑器输入";
  if (platform === "macos")
    return "输入法启用时按住修饰键快捷键录音，松开结束；组合键先按 Control。按住期间按空格锁定，Escape 取消。修饰键快捷键由输入法自身接收，不需要额外授权；Ctrl+F9 在输入法会话之外接收，需要在「系统设置 › 隐私与安全性 › 输入监控」中允许本输入法，否则按下没有任何反应。首次授权后请重新按键。";
  if (platform === "windows")
    return "输入法运行时全局生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，锁定后再按一次快捷键或点 ✓ 结束，Escape 或 ✗ 取消。Ctrl+F9 按一次开始、再按一次结束。";
  return "输入法运行时全局生效，用于开始和结束语音录音";
}

/** Platform-aware voice recording shortcut controls shared by desktop settings hosts. */
export function VoiceHotkeysSection({ platform, values, onChange }: VoiceHotkeysSectionProps) {
  return (
    <div className="section">
      <div className="section-title">
        语音快捷键
        <small>{description(platform)}</small>
      </div>
      {/* Linux hosts share the desktop labels; only their native key event handling differs. */}
      {hotkeyOptions(platform).map(([key, label]) => (
        <label className="section-header" key={key}>
          <span className="section-title">{label}</span>
          <input
            aria-label={label}
            className="toggle"
            type="checkbox"
            checked={values[key] !== false}
            onChange={(event) => onChange(key, event.target.checked)}
          />
        </label>
      ))}
    </div>
  );
}
