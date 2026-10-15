import type { ReactNode } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import { GroupList } from "../core/platform-controls";
import { SliderRow } from "./slider-row";
import { SwitchRow } from "./switch-row";
import { ActionRow } from "./action-row";
import type { SwipeSymbolsDirection } from "./mobile-keyboard-feedback";
import { SelectRow } from "./select-row";
import { touchKeyboardSpacingLimits } from "./touch-keyboard-spacing";
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
  tabletFullKeys?: boolean;
  /** 仅 iPad 宿主给出：横屏分离式键盘。没有时不画这个开关。 */
  tabletSplitKeyboard?: boolean;
  /** 宿主的键盘实现了滑行输入时给出当前开关；没有时不画这个开关。 */
  glideTyping?: boolean;
  /** 「滑动输入符号」开关及其方向，在宿主键盘实现了它们时传入；不传则两行都不绘制。 */
  swipeSymbols?: boolean;
  swipeSymbolsDirection?: SwipeSymbolsDirection;
  /** 宿主的触屏 26 键双拼会在字母键底部画键位提示时给出这个开关；没有时不画这一行。 */
  shuangpinKeyHints?: boolean;
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
  onShuangpinKeyHintsChange?: (enabled: boolean) => void;
  onReset: () => void;
  /** HarmonyOS 手机：「布局」组开头的行。给出时按手机的排法：「布局」只放键盘的选择，「尺寸」把预览、三个滑块和「恢复默认」放进同一张卡片，各行不带长说明。 */
  layoutRows?: ReactNode;
  /** HarmonyOS 手机：「尺寸」卡片顶部的键盘预览（不带分组），在它上面拖动调的就是下面的间距。 */
  preview?: ReactNode;
}

/** 间距以十分之一 dp 存，手机上显示成不带单位的数：6、6.5。 */
const spacingText = (tenths: number) => String(tenths / 10);
/** 高度调整以 dp 存，手机上 0 显示成「标准」，其余带正负号。 */
const heightText = (value: number) => (value === 0 ? "标准" : `${value > 0 ? "+" : ""}${value}`);

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
  onShuangpinKeyHintsChange,
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
  const resetDescription = showVoiceShortcut
    ? "高度、间距、顶部语音入口和工具栏按钮回到默认"
    : "高度、间距和工具栏按钮回到默认";

  return (
    <>
      {phone ? (
        <>
          {/* 双拼键位提示跟 Android 一样紧跟在「中文键盘」之后。 */}
          <GroupList title="布局">
            {layoutRows}
            {shuangpinKeyHintsRow}
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
              onClick={() => void onReset()}
            >
              恢复默认
            </button>
          </GroupList>
        </>
      ) : (
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
          {/* 重置一如既往也覆盖下面的「工具栏」组；它放在这里，也就是它所重置的第一组的末尾。 */}
          <ActionRow
            title="恢复默认"
            description={resetDescription}
            action={onReset}
            className="danger-text"
            ariaLabel="恢复屏幕键盘默认设置"
            label="恢复默认"
          />
        </GroupList>
      )}
      <TouchKeyboardToolbarSection
        touchVoiceShortcut={touchVoiceShortcut}
        voiceShortcutKind={voiceShortcutKind}
        toolbarComponents={toolbarComponents}
        toolbar={toolbar}
        onTouchVoiceShortcutChange={onTouchVoiceShortcutChange}
        onToolbarChange={onToolbarChange}
      />
      {(tabletFullKeys !== undefined ||
        tabletSplitKeyboard !== undefined ||
        (shuangpinKeyHints !== undefined && !phone)) && (
        <GroupList title="布局">
          {!phone && shuangpinKeyHintsRow}
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
