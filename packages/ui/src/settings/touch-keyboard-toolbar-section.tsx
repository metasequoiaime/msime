import { Checks, GroupList } from "../core/platform-controls";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsGroupNote } from "./settings-group-note";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";
import {
  harmonyRandomDigitsDescription,
  harmonyToolbarOptions,
  harmonyToolbarPreferences,
  type HarmonyToolbarPreferences,
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
  /** 宿主报告 `touch_toolbar_tools` 时（HarmonyOS 手机）给出 `harmony_toolbar` 的当前值，没存过时给空对象；不给时不画显示方式和工具开关。 */
  harmonyToolbar?: Partial<HarmonyToolbarPreferences>;
  /** 偏好里的方案是不是中文（`isChineseScheme(scheme)`），和 `harmonyToolbar` 一起给出。鸿蒙键盘只在中文方案上画「随」，所以只有这时「随机数字」能让空闲工具栏那一行留着；不给时按中文方案算。 */
  harmonyChineseScheme?: boolean;
  onHarmonyToolbarChange?: (value: HarmonyToolbarPreferences) => void;
}

/** 触屏键盘顶部语音入口和工具栏按钮设置，不支持的宿主不显示空分组。 */
export function TouchKeyboardToolbarSection({
  touchVoiceShortcut,
  voiceShortcutKind = "start-voice",
  toolbarComponents,
  toolbar,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  harmonyToolbar,
  harmonyChineseScheme = true,
  onHarmonyToolbarChange,
}: TouchKeyboardToolbarSectionProps) {
  const showVoiceShortcut = voiceShortcutKind !== "hidden";
  if (!showVoiceShortcut && !toolbarComponents && !harmonyToolbar) return null;
  const toolbarValues = touchToolbarPreferences(toolbar);
  const harmonyValues = harmonyToolbar && harmonyToolbarPreferences(harmonyToolbar);
  // 与鸿蒙键盘的 `TouchToolbarPolicy.rowHidden` 同一条规则：选了「隐藏」或工具全关时，空闲时整行不显示；「随」入口只在中文方案上算一个工具，只开着它时中文键盘的这一行照常显示，日文、韩文、越南文和藏文照旧不显示。
  const harmonyRowHidden =
    harmonyValues !== undefined &&
    (harmonyValues.hidden ||
      harmonyToolbarOptions.every(
        ([key]) => !harmonyValues[key] || (key === "random_digits" && !harmonyChineseScheme),
      ));
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
      {harmonyValues && (
        <>
          <SelectRow
            title="显示方式"
            aria-label="工具栏显示方式"
            value={harmonyValues.hidden ? "hidden" : "typing"}
            onChange={(event) =>
              onHarmonyToolbarChange?.({
                ...harmonyValues,
                hidden: event.target.value === "hidden",
              })
            }
          >
            <option value="typing">输入时显示</option>
            <option value="hidden">隐藏</option>
          </SelectRow>
          {harmonyToolbarOptions.map(([key, label]) => (
            <SwitchRow
              key={key}
              title={label}
              aria-label={`工具栏：${label}`}
              description={key === "random_digits" ? harmonyRandomDigitsDescription : undefined}
              checked={harmonyValues[key]}
              disabled={harmonyValues.hidden}
              onChange={(checked) => onHarmonyToolbarChange?.({ ...harmonyValues, [key]: checked })}
            />
          ))}
          {harmonyRowHidden && (
            <SettingsGroupNote>
              {harmonyValues.hidden
                ? "工具栏已隐藏：键盘变矮，打字时照常显示候选栏。"
                : "没有打开的工具，工具栏不显示：键盘变矮，打字时照常显示候选栏。"}
            </SettingsGroupNote>
          )}
        </>
      )}
    </GroupList>
  );
}
