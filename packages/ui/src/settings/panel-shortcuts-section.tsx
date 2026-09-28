import * as settings from "./settings-style";

export interface PanelShortcutsSectionProps {
  visible: boolean;
  macos: boolean;
  harmony: boolean;
}

/** Static panel shortcut guidance shared by desktop-capable hosts. */
export function PanelShortcutsSection({ visible, macos, harmony }: PanelShortcutsSectionProps) {
  if (!visible) return null;

  return (
    <div className="section" role="group" aria-label="面板快捷键">
      <div className="section-title">面板快捷键</div>
      <small>
        {macos
          ? "可从当前输入上下文使用 Command 组合键打开面板。"
          : harmony
            ? "连接实体键盘后，在输入状态下可用 Super 组合键打开面板。"
            : "桌面环境转发 Super 组合键时可从当前输入上下文打开面板。"}
      </small>
      <div className={settings.shortcutList}>
        <div className={settings.shortcutRow}>
          <span>打开屏幕键盘</span>
          <kbd>Ctrl+Shift+{macos ? "Command" : "Super"}+K</kbd>
        </div>
      </div>
    </div>
  );
}
