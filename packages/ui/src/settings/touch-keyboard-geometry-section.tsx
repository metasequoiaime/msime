import type { ReactNode } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import { GroupList } from "../core/platform-controls";
import { SliderRow } from "./slider-row";
import { SwitchRow } from "./switch-row";
import { ActionRow } from "./action-row";
import type { SwipeSymbolsDirection } from "./mobile-keyboard-feedback";
import { SelectRow } from "./select-row";
import {
  TouchSplitKeyboardRows,
  touchSplitKeyboardText,
  type TouchSplitKeyboardChange,
  type TouchSplitKeyboardSettings,
} from "./touch-split-keyboard-rows";
import type { Preferences } from "../core/preferences-model";
import { touchKeyboardSpacingLimits } from "./touch-keyboard-spacing";
import { touchKeyboardResetSubjects } from "./use-touch-keyboard-settings-reset";
import * as settings from "./settings-style";
import {
  TouchKeyboardToolbarSection,
  type TouchKeyboardToolbarSectionProps,
} from "./touch-keyboard-toolbar-section";

export type {
  TouchToolbarPreferences,
  TouchVoiceShortcutKind,
} from "./touch-keyboard-toolbar-preferences";

export interface TouchKeyboardGeometrySectionProps extends TouchKeyboardToolbarSectionProps {
  heightAdjustment: number;
  keySpacingTenths: number;
  rowSpacingTenths: number;
  /** 宿主的键盘按 `touch_keyboard_height_adjustment` 改变高度时为 true（缺省）；macOS 和 Linux 的键盘窗口高度固定，不画「键盘高度」。 */
  showHeight?: boolean;
  /** 宿主的键盘按两个间距值画键距和行距时为 true（缺省）；macOS 固定用默认间距，不画两个间距滑块。高度和间距都不画时整个「尺寸」组连同「恢复默认」一起不画。 */
  showSpacing?: boolean;
  tabletFullKeys?: boolean;
  /** 仅 iPad 宿主给出：横屏分离式键盘。没有时不画这个开关。 */
  tabletSplitKeyboard?: boolean;
  /** 宿主的触屏键盘在大屏上画分离式键盘（`HostCapabilities.touch_keyboard_split`）时给出四项共享偏好的当前值；没有时不画这几行。 */
  splitKeyboard?: TouchSplitKeyboardSettings;
  /** 宿主的键盘实现了滑行输入时给出当前开关；没有时不画这个开关。 */
  glideTyping?: boolean;
  /** 「滑动输入符号」开关及其方向，在宿主键盘实现了它们时传入；不传则两行都不绘制。 */
  swipeSymbols?: boolean;
  swipeSymbolsDirection?: SwipeSymbolsDirection;
  /** 「显示字母键角标」开关，在宿主键盘实现了它时传入；不传则不画这一行。 */
  letterCornerHints?: boolean;
  /** 「九键滑动输入数字」（共享偏好 `touch_nine_key_swipe_digits`），宿主的触屏键盘提供拼音九键时给出当前值（缺键和不认识的值由调用方读成 `off`）；不传则不画这一行。 */
  nineKeySwipeDigits?: NineKeySwipeDigits;
  /** 宿主的触屏 26 键双拼会在字母键底部画键位提示时给出这个开关；没有时不画这一行。 */
  shuangpinKeyHints?: boolean;
  /** 宿主的触屏全拼 26 键组字会保留 ⇧ 臂辅助码（`touch_quanpin_shift`）时给出这个开关；没有时不画这一行。 */
  quanpinShift?: boolean;
  /** 宿主的触屏键盘在大屏单手模式空出来的一侧画数字键盘（`HostCapabilities.touch_one_handed_side_panel`）时给出：`touch_one_handed_side_panel` 是不是 `number_pad`。没有时不画这一行。 */
  oneHandedSidePanel?: boolean;
  /** 键盘本机设置正在保存时为 true，「数字行与 Tab 键」「横屏分离式键盘」和「滑行输入」都暂不可点。 */
  tabletFullKeysBusy: boolean;
  onHeightAdjustmentChange: (value: number) => void;
  onKeySpacingChange: (value: number) => void;
  onRowSpacingChange: (value: number) => void;
  onTabletFullKeysChange: (enabled: boolean) => void;
  onTabletSplitKeyboardChange: (enabled: boolean) => void;
  onGlideTypingChange: (enabled: boolean) => void;
  onSwipeSymbolsChange?: (enabled: boolean) => void;
  onSwipeSymbolsDirectionChange?: (direction: SwipeSymbolsDirection) => void;
  onLetterCornerHintsChange?: (enabled: boolean) => void;
  onNineKeySwipeDigitsChange?: (value: NineKeySwipeDigits) => void;
  onShuangpinKeyHintsChange?: (enabled: boolean) => void;
  onQuanpinShiftChange?: (enabled: boolean) => void;
  onOneHandedSidePanelChange?: (enabled: boolean) => void;
  onSplitKeyboardChange?: (change: TouchSplitKeyboardChange) => void;
  /** 「恢复默认」：参数是这一页实际给出、会被恢复的几项，如「高度、间距和工具栏按钮」，确认框照它措辞。 */
  onReset: (subjects: string) => void;
  /** HarmonyOS 手机：「布局」组开头的行。给出时按手机的排法：「布局」只放键盘的选择，「尺寸」把预览、三个滑块和「恢复默认」放进同一张卡片，各行不带长说明。 */
  layoutRows?: ReactNode;
  /** HarmonyOS 手机：「尺寸」卡片顶部的键盘预览（不带分组），在它上面拖动调的就是下面的间距。 */
  preview?: ReactNode;
}

/** 「大屏单手时侧边显示数字键盘」的标题和说明，三端逐字相同（Android `KeyboardOptionsPage`、iOS `KeyboardLayoutSettingsView`）。 */
const oneHandedSidePanelTitle = "大屏单手时侧边显示数字键盘";
const oneHandedSidePanelDescription =
  "单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。";

/** 共享偏好 `touch_nine_key_swipe_digits` 的取值：关闭、上滑输入数字（下滑弹出字母）、下滑输入数字（上滑弹出字母）。 */
export type NineKeySwipeDigits = NonNullable<Preferences["touch_nine_key_swipe_digits"]>;

/** 间距以十分之一 dp 存，手机上显示成不带单位的数：6、6.5。 */
const spacingText = (tenths: number) => String(tenths / 10);
/** 高度调整以 dp 存，手机上 0 显示成「标准」，其余带正负号。 */
const heightText = (value: number) => (value === 0 ? "标准" : `${value > 0 ? "+" : ""}${value}`);

/** 「屏幕键盘」页共用的触屏键盘控件，即预览之后的各组：「尺寸」（高度、间距和重置）、「工具栏」，宿主有 iPad 数字行和 Tab 键或横屏分离式键盘时的「布局」，以及宿主有滑行输入时的「手势」。 */
export function TouchKeyboardGeometrySection({
  heightAdjustment,
  keySpacingTenths,
  rowSpacingTenths,
  showHeight = true,
  showSpacing = true,
  touchVoiceShortcut,
  voiceShortcutKind = "start-voice",
  toolbarComponents,
  toolbar,
  harmonyToolbar,
  tabletFullKeys,
  tabletSplitKeyboard,
  glideTyping,
  swipeSymbols,
  swipeSymbolsDirection = "up",
  letterCornerHints,
  nineKeySwipeDigits,
  shuangpinKeyHints,
  quanpinShift,
  oneHandedSidePanel,
  splitKeyboard,
  tabletFullKeysBusy,
  onHeightAdjustmentChange,
  onKeySpacingChange,
  onRowSpacingChange,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  onHarmonyToolbarChange,
  onTabletFullKeysChange,
  onTabletSplitKeyboardChange,
  onGlideTypingChange,
  onSwipeSymbolsChange,
  onSwipeSymbolsDirectionChange,
  onLetterCornerHintsChange,
  onNineKeySwipeDigitsChange,
  onShuangpinKeyHintsChange,
  onQuanpinShiftChange,
  onOneHandedSidePanelChange,
  onSplitKeyboardChange,
  onReset,
  layoutRows,
  preview,
}: TouchKeyboardGeometrySectionProps) {
  const showVoiceShortcut = voiceShortcutKind !== "hidden";
  // HarmonyOS 手机一屏只有约 360px 宽，每行挂两三行说明会把页面撑得又长又乱，所以那里的行只留标题和取值。
  const phone = Boolean(layoutRows);
  const shuangpinKeyHintsRow = shuangpinKeyHints !== undefined && (
    <SwitchRow
      title="双拼键位提示"
      description={
        phone
          ? undefined
          : "双拼方案的 26 键键盘在字母键底部显示这个键代表的声母和韵母；关闭后键面只留字母。"
      }
      checked={shuangpinKeyHints}
      onChange={(enabled) => onShuangpinKeyHintsChange?.(enabled)}
    />
  );
  const quanpinShiftRow = quanpinShift !== undefined && (
    <SwitchRow
      title="组字时保留 Shift"
      description={
        phone
          ? undefined
          : "全拼 26 键组字时左下角保留 ⇧，点一下再输字母即辅助码；默认开启，关闭后这一键改为「分词」，用来断开音节。"
      }
      checked={quanpinShift}
      onChange={(enabled) => onQuanpinShiftChange?.(enabled)}
    />
  );
  // 标题和说明与 Android、iOS 原生设置页逐字相同。手机排法里也带说明：这一行只在平板和可折叠手机上出现，不在窄屏上挤；没有说明时，打开后在 26 键或双手模式下看不到任何变化，也不知道为什么。
  const oneHandedSidePanelRow = oneHandedSidePanel !== undefined && (
    <SwitchRow
      title={oneHandedSidePanelTitle}
      description={oneHandedSidePanelDescription}
      checked={oneHandedSidePanel}
      onChange={(enabled) => onOneHandedSidePanelChange?.(enabled)}
    />
  );
  // 分离式键盘的四行与 Android 原生设置页逐字相同；手机排法里也带说明，理由同上：只在平板和可折叠手机上出现。
  const splitKeyboardRows = splitKeyboard !== undefined && (
    <TouchSplitKeyboardRows
      value={splitKeyboard}
      onChange={(change) => onSplitKeyboardChange?.(change)}
    />
  );
  // 只列这一页上看得到的几项：工具栏按钮只在触屏宿主的「工具栏」组里出现，以前桌面上也写着它。
  const resetSubjects = touchKeyboardResetSubjects({
    height: showHeight,
    spacing: showSpacing,
    voiceShortcut: showVoiceShortcut,
    toolbar: toolbarComponents || harmonyToolbar !== undefined,
  });
  const resetDescription = `${resetSubjects}回到默认`;

  return (
    <>
      {phone ? (
        <>
          {/* 双拼键位提示跟 Android 一样紧跟在「中文键盘」之后。 */}
          <GroupList title="布局">
            {layoutRows}
            {shuangpinKeyHintsRow}
            {quanpinShiftRow}
            {splitKeyboardRows}
            {oneHandedSidePanelRow}
          </GroupList>
          {/* 预览和它调的三个滑块在同一张卡片里；「恢复默认」是卡片末尾的一行文字按钮，它也照旧重置工具栏按钮。 */}
          <GroupList title="尺寸">
            {preview}
            <SliderRow
              title="键盘高度"
              wide
              displayValue={heightText(heightAdjustment)}
              valueText={`${heightAdjustment} dp`}
              min={-12}
              max={48}
              value={heightAdjustment}
              onChange={onHeightAdjustmentChange}
            />
            <SliderRow
              title="按键间距"
              wide
              displayValue={spacingText(keySpacingTenths)}
              valueText={`${spacingText(keySpacingTenths)} dp`}
              min={touchKeyboardSpacingLimits.key.min}
              max={touchKeyboardSpacingLimits.key.max}
              value={keySpacingTenths}
              onChange={onKeySpacingChange}
            />
            <SliderRow
              title="行间距"
              wide
              displayValue={spacingText(rowSpacingTenths)}
              valueText={`${spacingText(rowSpacingTenths)} dp`}
              min={touchKeyboardSpacingLimits.row.min}
              max={touchKeyboardSpacingLimits.row.max}
              value={rowSpacingTenths}
              onChange={onRowSpacingChange}
            />
            <button
              type="button"
              className={settings.centeredDangerRow}
              aria-label="恢复屏幕键盘默认设置"
              onClick={() => onReset(resetSubjects)}
            >
              恢复默认
            </button>
          </GroupList>
        </>
      ) : (
        (showHeight || showSpacing) && (
          <GroupList title="尺寸">
            <SettingsGroupNote>
              {showSpacing
                ? "只改变触屏键位的外观，不改变输入方案；也可以直接在上方预览上左右拖动调节键距、上下拖动调节行距。"
                : "只改变触屏键位的外观，不改变输入方案。"}
            </SettingsGroupNote>
            {showHeight && (
              <SliderRow
                title="键盘高度"
                description={`${heightAdjustment > 0 ? "+" : ""}${heightAdjustment} dp`}
                min={-12}
                max={48}
                value={heightAdjustment}
                onChange={onHeightAdjustmentChange}
              />
            )}
            {showSpacing && (
              <>
                <SliderRow
                  title="按键间距"
                  description={`${(keySpacingTenths / 10).toFixed(1)} dp`}
                  min={touchKeyboardSpacingLimits.key.min}
                  max={touchKeyboardSpacingLimits.key.max}
                  value={keySpacingTenths}
                  onChange={onKeySpacingChange}
                />
                <SliderRow
                  title="行间距"
                  description={`${(rowSpacingTenths / 10).toFixed(1)} dp`}
                  min={touchKeyboardSpacingLimits.row.min}
                  max={touchKeyboardSpacingLimits.row.max}
                  value={rowSpacingTenths}
                  onChange={onRowSpacingChange}
                />
              </>
            )}
            {/* 重置一如既往也覆盖下面的「工具栏」组；它放在这里，也就是它所重置的第一组的末尾。 */}
            <ActionRow
              title="恢复默认"
              description={resetDescription}
              action={() => onReset(resetSubjects)}
              className="danger-text"
              ariaLabel="恢复屏幕键盘默认设置"
              label="恢复默认"
            />
          </GroupList>
        )
      )}
      <TouchKeyboardToolbarSection
        touchVoiceShortcut={touchVoiceShortcut}
        voiceShortcutKind={voiceShortcutKind}
        toolbarComponents={toolbarComponents}
        toolbar={toolbar}
        onTouchVoiceShortcutChange={onTouchVoiceShortcutChange}
        onToolbarChange={onToolbarChange}
        harmonyToolbar={harmonyToolbar}
        onHarmonyToolbarChange={onHarmonyToolbarChange}
      />
      {(tabletFullKeys !== undefined ||
        tabletSplitKeyboard !== undefined ||
        (shuangpinKeyHints !== undefined && !phone) ||
        (quanpinShift !== undefined && !phone) ||
        (splitKeyboard !== undefined && !phone) ||
        (oneHandedSidePanel !== undefined && !phone)) && (
        <GroupList title="布局">
          {!phone && shuangpinKeyHintsRow}
          {!phone && quanpinShiftRow}
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
              title={touchSplitKeyboardText.title}
              description="只在 iPad 横屏时生效：字母、数字行和 123 符号页从中间分成左右两半，方便双手握持时用拇指打字；九键、笔画、手写和注音不分，竖屏、浮动键盘和窄窗口照常显示整块键盘。"
              disabled={tabletFullKeysBusy}
              checked={tabletSplitKeyboard}
              onChange={onTabletSplitKeyboardChange}
            />
          )}
          {!phone && splitKeyboardRows}
          {!phone && oneHandedSidePanelRow}
        </GroupList>
      )}
      {(glideTyping !== undefined ||
        swipeSymbols !== undefined ||
        letterCornerHints !== undefined ||
        nineKeySwipeDigits !== undefined) && (
        <GroupList title="手势">
          {swipeSymbols !== undefined && (
            <>
              <SwitchRow
                title="滑动输入符号"
                description={
                  phone
                    ? "在字母键上滑动输入角标符号"
                    : "在字母键上滑动，输入角标符号；长按字母键始终可以输入"
                }
                disabled={tabletFullKeysBusy}
                checked={swipeSymbols}
                onChange={(enabled) => onSwipeSymbolsChange?.(enabled)}
              />
              <SelectRow
                title="滑动方向"
                disabled={tabletFullKeysBusy || !swipeSymbols}
                value={swipeSymbolsDirection}
                onChange={(event) =>
                  onSwipeSymbolsDirectionChange?.(event.target.value as SwipeSymbolsDirection)
                }
              >
                <option value="down">下滑</option>
                <option value="up">上滑</option>
              </SelectRow>
            </>
          )}
          {/* 只管画不画，不跟着「滑动输入符号」置灰：关掉滑动后长按仍输入角标，键面上的角标仍有用（#10587）。与 Android、iOS 的「手势」组同一位置。 */}
          {letterCornerHints !== undefined && (
            <SwitchRow
              title="显示字母键角标"
              description="26 键字母键右上角显示长按或滑动输入的符号；关闭后只是不显示，长按和滑动照样输入"
              disabled={tabletFullKeysBusy}
              checked={letterCornerHints}
              onChange={(enabled) => onLetterCornerHintsChange?.(enabled)}
            />
          )}
          {/* 紧跟 26 键的「滑动方向」，与 Android、iOS 的「手势」组同一位置；它只写共享偏好，和 26 键开关、本机设置的保存都无关，不跟着置灰。 */}
          {nineKeySwipeDigits !== undefined && (
            <SelectRow
              title="九键滑动输入数字"
              description="沿所选方向滑动输入数字，反方向弹出字母。"
              value={nineKeySwipeDigits}
              onChange={(event) =>
                onNineKeySwipeDigitsChange?.(event.target.value as NineKeySwipeDigits)
              }
            >
              <option value="off">关闭</option>
              <option value="up">上滑</option>
              <option value="down">下滑</option>
            </SelectRow>
          )}
          {glideTyping !== undefined && (
            <SwitchRow
              title="滑行输入"
              description={
                phone
                  ? "只在全拼 26 键生效"
                  : "在字母键上连续滑动输入拼音，停留可确认经过的键；只在全拼 26 键的字母面生效。"
              }
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
