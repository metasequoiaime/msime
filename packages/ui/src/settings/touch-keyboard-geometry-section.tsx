import { SettingsGroupNote } from "./settings-group-note";
import { Checks, GroupList } from "../core/platform-controls";
import { SliderRow } from "./slider-row";
import { SwitchRow } from "./switch-row";
import { ActionRow } from "./action-row";
import { SettingsGroupBlock } from "./settings-group-block";

export type TouchToolbarPreferences = {
  layout: boolean;
  emoji: boolean;
  skin: boolean;
  clipboard: boolean;
  ai: boolean;
  character_set: boolean;
  fullwidth: boolean;
  punctuation: boolean;
};

const defaultTouchToolbar: TouchToolbarPreferences = {
  layout: true,
  emoji: true,
  skin: true,
  clipboard: false,
  ai: false,
  character_set: false,
  fullwidth: false,
  punctuation: false,
};

/// In the order the buttons sit on the touch keyboard's toolbar, after the voice entry.
const touchToolbarOptions: readonly [keyof TouchToolbarPreferences, string][] = [
  ["layout", "键盘设置"],
  ["emoji", "表情"],
  ["skin", "切换皮肤"],
  ["clipboard", "剪贴板历史"],
  ["ai", "AI 润色"],
  ["character_set", "简繁切换"],
  ["fullwidth", "全角 / 半角"],
  ["punctuation", "中英文标点"],
];

/** How the host's keyboard treats the voice button at the top: most hosts start voice input from it, the iOS keyboard extension opens the last result recognised in the app, and the macOS desktop keyboard does not draw it. */
export type TouchVoiceShortcutKind = "start-voice" | "last-result" | "hidden";

export interface TouchKeyboardGeometrySectionProps {
  heightAdjustment: number;
  keySpacingTenths: number;
  rowSpacingTenths: number;
  touchVoiceShortcut: boolean;
  voiceShortcutKind?: TouchVoiceShortcutKind;
  toolbarComponents: boolean;
  toolbar?: Partial<TouchToolbarPreferences>;
  tabletFullKeys?: boolean;
  /** 仅 iPad 宿主给出：横屏分离式键盘。没有时不画这个开关。 */
  tabletSplitKeyboard?: boolean;
  /** iPad 布局开关正在保存时为 true，「数字行与 Tab 键」和「横屏分离式键盘」都暂不可点。 */
  tabletFullKeysBusy: boolean;
  onHeightAdjustmentChange: (value: number) => void;
  onKeySpacingChange: (value: number) => void;
  onRowSpacingChange: (value: number) => void;
  onTouchVoiceShortcutChange: (enabled: boolean) => void;
  onToolbarChange: (value: TouchToolbarPreferences) => void;
  onTabletFullKeysChange: (enabled: boolean) => void;
  onTabletSplitKeyboardChange: (enabled: boolean) => void;
  onReset: () => void;
}

/** 「屏幕键盘」页共用的触屏键盘控件，即预览之后的各组：「尺寸」（高度、间距和重置）、「工具栏」，以及宿主有 iPad 数字行和 Tab 键或横屏分离式键盘时的「布局」。 */
export function TouchKeyboardGeometrySection({
  heightAdjustment,
  keySpacingTenths,
  rowSpacingTenths,
  touchVoiceShortcut,
  voiceShortcutKind = "start-voice",
  toolbarComponents,
  toolbar,
  tabletFullKeys,
  tabletSplitKeyboard,
  tabletFullKeysBusy,
  onHeightAdjustmentChange,
  onKeySpacingChange,
  onRowSpacingChange,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  onTabletFullKeysChange,
  onTabletSplitKeyboardChange,
  onReset,
}: TouchKeyboardGeometrySectionProps) {
  const toolbarValues = { ...defaultTouchToolbar, ...toolbar };
  const showVoiceShortcut = voiceShortcutKind !== "hidden";

  return (
    <>
      <GroupList title="尺寸">
        <SettingsGroupNote>
          只改变触屏键位的外观，不改变输入方案；也可以直接在上方预览上左右拖动调节键距、上下拖动调节行距。
        </SettingsGroupNote>
        <SliderRow
          title="键盘高度"
          description={`${heightAdjustment > 0 ? "+" : ""}${heightAdjustment} dp`}
          min={-12}
          max={48}
          value={heightAdjustment}
          onChange={onHeightAdjustmentChange}
        />
        <SliderRow
          title="按键间距"
          description={`${(keySpacingTenths / 10).toFixed(1)} dp`}
          min={30}
          max={60}
          value={keySpacingTenths}
          onChange={onKeySpacingChange}
        />
        <SliderRow
          title="行间距"
          description={`${(rowSpacingTenths / 10).toFixed(1)} dp`}
          min={40}
          max={100}
          value={rowSpacingTenths}
          onChange={onRowSpacingChange}
        />
        {/* 重置一如既往也覆盖下面的「工具栏」组；它放在这里，也就是它所重置的第一组的末尾。 */}
        <ActionRow
          title="恢复默认"
          description={
            showVoiceShortcut
              ? "高度、间距、顶部语音入口和工具栏按钮回到默认"
              : "高度、间距和工具栏按钮回到默认"
          }
          action={onReset}
          className="danger-text"
          ariaLabel="恢复屏幕键盘默认设置"
          label="恢复默认"
        />
      </GroupList>
      {(showVoiceShortcut || toolbarComponents) && (
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
                  // The hidden prefix keeps each box named "工具栏：…" for assistive technology, as the old per-box label did.
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
      )}
      {(tabletFullKeys !== undefined || tabletSplitKeyboard !== undefined) && (
        <GroupList title="布局">
          {tabletFullKeys !== undefined && (
            <SwitchRow
              title="数字行与 Tab 键"
              description="iPad 全宽键盘在字母上方显示数字行，并在 Q 左侧显示 Tab 键；浮动键盘和窄窗口没有空间，不显示。"
              disabled={tabletFullKeysBusy}
              checked={tabletFullKeys}
              onChange={onTabletFullKeysChange}
            />
          )}
          {tabletSplitKeyboard !== undefined && (
            <SwitchRow
              title="横屏分离式键盘"
              description="只在 iPad 横屏时生效：字母、数字行和 123 符号页从中间分成左右两半，方便双手握持时用拇指打字；九键、笔画、手写和注音不分，竖屏、浮动键盘和窄窗口照常显示整块键盘。"
              disabled={tabletFullKeysBusy}
              checked={tabletSplitKeyboard}
              onChange={onTabletSplitKeyboardChange}
            />
          )}
        </GroupList>
      )}
    </>
  );
}
