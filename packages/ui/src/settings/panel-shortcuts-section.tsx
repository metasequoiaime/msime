import { SettingsGroupNote } from "./settings-group-note";
import { SettingsRowStack } from "./settings-row-stack";
import { GroupList } from "../core/platform-controls";
import { ShortcutRow } from "./shortcut-row";

export interface PanelShortcutsSectionProps {
  visible: boolean;
  macos: boolean;
  harmony: boolean;
}

/** 「打开屏幕键盘」一行，说明这个组合键在哪些情况下可用。桌面快捷键页把它放进「按键速查」，不再单占一组。 */
export function PanelShortcutRow({ macos, harmony }: Omit<PanelShortcutsSectionProps, "visible">) {
  return (
    <ShortcutRow
      title="打开屏幕键盘"
      description={panelShortcutNote(macos, harmony)}
      chord={`Ctrl+Shift+${macos ? "Command" : "Super"}+K`}
    />
  );
}

function panelShortcutNote(macos: boolean, harmony: boolean) {
  return macos
    ? "可从当前输入上下文使用 Command 组合键打开面板。"
    : harmony
      ? "连接实体键盘后，在输入状态下可用 Super 组合键打开面板。"
      : "桌面环境转发 Super 组合键时可从当前输入上下文打开面板。";
}

/** Static panel shortcut guidance shared by desktop-capable hosts. */
export function PanelShortcutsSection({ visible, macos, harmony }: PanelShortcutsSectionProps) {
  if (!visible) return null;

  return (
    <GroupList title="面板快捷键">
      <SettingsRowStack role="group" aria-label="面板快捷键">
        <SettingsGroupNote>{panelShortcutNote(macos, harmony)}</SettingsGroupNote>
        <ShortcutRow title="打开屏幕键盘" chord={`Ctrl+Shift+${macos ? "Command" : "Super"}+K`} />
      </SettingsRowStack>
    </GroupList>
  );
}
