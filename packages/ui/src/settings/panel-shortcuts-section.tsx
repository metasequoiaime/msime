import * as settings from "./settings-style";
import { GroupList, Row } from "../core/platform-controls";

export interface PanelShortcutsSectionProps {
  visible: boolean;
  macos: boolean;
  harmony: boolean;
}

/** Static panel shortcut guidance shared by desktop-capable hosts. */
export function PanelShortcutsSection({ visible, macos, harmony }: PanelShortcutsSectionProps) {
  if (!visible) return null;

  return (
    <GroupList title="面板快捷键">
      <div className={settings.rowStack} role="group" aria-label="面板快捷键">
        <p className={settings.groupNote}>
          {macos
            ? "可从当前输入上下文使用 Command 组合键打开面板。"
            : harmony
              ? "连接实体键盘后，在输入状态下可用 Super 组合键打开面板。"
              : "桌面环境转发 Super 组合键时可从当前输入上下文打开面板。"}
        </p>
        <Row title="打开屏幕键盘">
          <kbd className={settings.shortcutKey}>Ctrl+Shift+{macos ? "Command" : "Super"}+K</kbd>
        </Row>
      </div>
    </GroupList>
  );
}
