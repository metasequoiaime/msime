import { Checks, GroupList } from "../core/platform-controls";
import { SettingsGroupBlock } from "./settings-group-block";
import { SwitchRow } from "./switch-row";
import {
  touchToolbarOptions,
  touchToolbarPreferences,
  type TouchToolbarPreferences,
  type TouchVoiceShortcutKind,
} from "./touch-keyboard-toolbar-preferences";

export interface TouchKeyboardToolbarSectionProps {
  touchVoiceShortcut: boolean;
  voiceShortcutKind?: TouchVoiceShortcutKind;
  toolbarComponents: boolean;
  toolbar?: Partial<TouchToolbarPreferences>;
  onTouchVoiceShortcutChange: (enabled: boolean) => void;
  onToolbarChange: (value: TouchToolbarPreferences) => void;
}

/** 触屏键盘顶部语音入口和工具栏按钮设置，不支持的宿主不显示空分组。 */
export function TouchKeyboardToolbarSection({
  touchVoiceShortcut,
  voiceShortcutKind = "start-voice",
  toolbarComponents,
  toolbar,
  onTouchVoiceShortcutChange,
  onToolbarChange,
}: TouchKeyboardToolbarSectionProps) {
  const showVoiceShortcut = voiceShortcutKind !== "hidden";
  if (!showVoiceShortcut && !toolbarComponents) return null;
  const toolbarValues = touchToolbarPreferences(toolbar);
  return (
    <GroupList title="工具栏">
      {showVoiceShortcut && (
        <SwitchRow
          title="顶部语音入口"
          description={
            voiceShortcutKind === "last-result"
              ? "在触屏键盘工具栏直接打开最近一次语音结果"
              : "在键盘顶部显示一个语音按钮，点按开始语音输入"
          }
          checked={touchVoiceShortcut}
          onChange={onTouchVoiceShortcutChange}
        />
      )}
      {toolbarComponents && (
        <SettingsGroupBlock>
          <Checks
            legend="工具栏按钮"
            description="勾选要显示在键盘顶部工具栏的功能；未勾选的仍在「更多」里"
            items={touchToolbarOptions.map(([key, label]) => ({
              value: key,
              // 隐藏前缀保留辅助技术读出的「工具栏：…」名称。
              label: (
                <>
                  <span className="sr-only">工具栏：</span>
                  {label}
                </>
              ),
              checked: toolbarValues[key],
            }))}
            onChange={(key, checked) => onToolbarChange({ ...toolbarValues, [key]: checked })}
          />
        </SettingsGroupBlock>
      )}
    </GroupList>
  );
}
