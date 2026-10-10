import type { ReactNode } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import { Checks, GroupList, Row, Select, type SegmentedOption } from "../core/platform-controls";
import { SliderRow } from "./slider-row";
import { SwitchRow } from "./switch-row";
import { ActionRow } from "./action-row";
import { SettingsGroupBlock } from "./settings-group-block";
import type { SwipeSymbolsDirection } from "./mobile-keyboard-feedback-section";
import { SegmentedRow } from "./segmented-row";

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
  /** 宿主的键盘实现了滑行输入时给出当前开关；没有时不画这个开关。 */
  glideTyping?: boolean;
  /** 「滑动输入符号」开关及其方向，在宿主键盘实现了它们时传入；不传则两行都不绘制。 */
  swipeSymbols?: boolean;
  swipeSymbolsDirection?: SwipeSymbolsDirection;
  /** 宿主有触屏九宫格数字层时给出它的排列；没有时不画这一行。 */
  numberKeypadOrder?: "phone" | "calculator";
  /** 宿主的 26 键能把「123」换成九宫格数字层时给出当前选择；没有时不画这一行。 */
  twentySixKeyNumberLayout?: "row" | "nine_key";
  /** 宿主的触屏 26 键双拼会在字母键底部画键位提示时给出这个开关；没有时不画这一行。 */
  shuangpinKeyHints?: boolean;
  /** 键盘本机设置正在保存时为 true，「数字行与 Tab 键」「横屏分离式键盘」和「滑行输入」都暂不可点。 */
  tabletFullKeysBusy: boolean;
  onHeightAdjustmentChange: (value: number) => void;
  onKeySpacingChange: (value: number) => void;
  onRowSpacingChange: (value: number) => void;
  onTouchVoiceShortcutChange: (enabled: boolean) => void;
  onToolbarChange: (value: TouchToolbarPreferences) => void;
  onTabletFullKeysChange: (enabled: boolean) => void;
  onTabletSplitKeyboardChange: (enabled: boolean) => void;
  onGlideTypingChange: (enabled: boolean) => void;
  onSwipeSymbolsChange?: (enabled: boolean) => void;
  onSwipeSymbolsDirectionChange?: (direction: SwipeSymbolsDirection) => void;
  onNumberKeypadOrderChange?: (value: "phone" | "calculator") => void;
  onTwentySixKeyNumberLayoutChange?: (value: "row" | "nine_key") => void;
  onShuangpinKeyHintsChange?: (enabled: boolean) => void;
  onReset: () => void;
  /** HarmonyOS 手机：放在尺寸组开头的行，此时该组按手机设计稿命名为「布局」。 */
  layoutRows?: ReactNode;
  /** HarmonyOS 手机：键盘预览，紧接尺寸组之后绘制，而不是像其他地方那样画在它上方。 */
  preview?: ReactNode;
}

const NUMBER_KEYPAD_ORDERS: readonly SegmentedOption<"phone" | "calculator">[] = [
  { value: "phone", label: "电话" },
  { value: "calculator", label: "计算器" },
];

const TWENTY_SIX_KEY_NUMBER_LAYOUTS: readonly SegmentedOption<"row" | "nine_key">[] = [
  { value: "row", label: "一行" },
  { value: "nine_key", label: "九宫格" },
];

/** 「屏幕键盘」页共用的触屏键盘控件，即预览之后的各组：「尺寸」（高度、间距和重置）、「工具栏」，宿主有 iPad 数字行和 Tab 键或横屏分离式键盘时的「布局」，以及宿主有滑行输入时的「手势」。 */
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
  glideTyping,
  swipeSymbols,
  swipeSymbolsDirection = "down",
  numberKeypadOrder,
  twentySixKeyNumberLayout,
  shuangpinKeyHints,
  tabletFullKeysBusy,
  onHeightAdjustmentChange,
  onKeySpacingChange,
  onRowSpacingChange,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  onTabletFullKeysChange,
  onTabletSplitKeyboardChange,
  onGlideTypingChange,
  onSwipeSymbolsChange,
  onSwipeSymbolsDirectionChange,
  onNumberKeypadOrderChange,
  onTwentySixKeyNumberLayoutChange,
  onShuangpinKeyHintsChange,
  onReset,
  layoutRows,
  preview,
}: TouchKeyboardGeometrySectionProps) {
  const toolbarValues = { ...defaultTouchToolbar, ...toolbar };
  const showVoiceShortcut = voiceShortcutKind !== "hidden";
  const numberKeypadRow = numberKeypadOrder !== undefined && (
    <>
      {twentySixKeyNumberLayout !== undefined && (
        <SegmentedRow
          title="26 键数字键盘"
          description="26 键按 123 时的数字键盘：一行把 1 到 0 排在符号上面，九宫格换成和九键一样的 3×3 数字键。"
          options={TWENTY_SIX_KEY_NUMBER_LAYOUTS}
          value={twentySixKeyNumberLayout}
          onChange={(value) => onTwentySixKeyNumberLayoutChange?.(value)}
        />
      )}
      <SegmentedRow
        title="数字键盘顺序"
        description="九宫格数字键盘的排列：电话把 1 2 3 放在最上面，计算器把 7 8 9 放在最上面。"
        options={NUMBER_KEYPAD_ORDERS}
        value={numberKeypadOrder}
        onChange={(value) => onNumberKeypadOrderChange?.(value)}
      />
    </>
  );
  const shuangpinKeyHintsRow = shuangpinKeyHints !== undefined && (
    <SwitchRow
      title="双拼键位提示"
      description="双拼方案的 26 键键盘在字母键底部显示这个键代表的声母和韵母；关闭后键面只留字母。"
      checked={shuangpinKeyHints}
      onChange={(enabled) => onShuangpinKeyHintsChange?.(enabled)}
    />
  );
  // HarmonyOS 手机的尺寸组本身就叫「布局」，数字键盘顺序和双拼键位提示跟 Android 一样紧跟在「中文键盘」之后，不再另起一个同名分组。
  const numberKeypadInLayoutRows = Boolean(layoutRows) && numberKeypadRow;
  const shuangpinKeyHintsInLayoutRows = Boolean(layoutRows) && shuangpinKeyHintsRow;

  return (
    <>
      <GroupList title={layoutRows ? "布局" : "尺寸"}>
        {layoutRows}
        {numberKeypadInLayoutRows}
        {shuangpinKeyHintsInLayoutRows}
        <SettingsGroupNote>
          {layoutRows
            ? `高度和间距只改变触屏键位的外观；也可以直接在${preview ? "下方" : "上方"}预览上左右拖动调节键距、上下拖动调节行距。`
            : `只改变触屏键位的外观，不改变输入方案；也可以直接在${preview ? "下方" : "上方"}预览上左右拖动调节键距、上下拖动调节行距。`}
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
      {preview}
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
      {(tabletFullKeys !== undefined ||
        tabletSplitKeyboard !== undefined ||
        ((numberKeypadOrder !== undefined || shuangpinKeyHints !== undefined) && !layoutRows)) && (
        <GroupList title="布局">
          {!layoutRows && numberKeypadRow}
          {!layoutRows && shuangpinKeyHintsRow}
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
      {(glideTyping !== undefined || swipeSymbols !== undefined) && (
        <GroupList title="手势">
          {swipeSymbols !== undefined && (
            <>
              <SwitchRow
                title="滑动输入符号"
                description="在字母键上滑动，输入角标符号；长按字母键始终可以输入"
                disabled={tabletFullKeysBusy}
                checked={swipeSymbols}
                onChange={(enabled) => onSwipeSymbolsChange?.(enabled)}
              />
              <Row title="滑动方向">
                <Select
                  disabled={tabletFullKeysBusy || !swipeSymbols}
                  value={swipeSymbolsDirection}
                  onChange={(event) =>
                    onSwipeSymbolsDirectionChange?.(event.target.value as SwipeSymbolsDirection)
                  }
                >
                  <option value="down">下滑</option>
                  <option value="up">上滑</option>
                </Select>
              </Row>
            </>
          )}
          {glideTyping !== undefined && (
            <SwitchRow
              title="滑行输入"
              description="在字母键上连续滑动输入拼音，停留可确认经过的键；只在全拼 26 键的字母面生效。"
              disabled={tabletFullKeysBusy}
              checked={glideTyping}
              onChange={onGlideTypingChange}
            />
          )}
        </GroupList>
      )}
    </>
  );
}
