import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";
import { keyboardThemeId } from "../../theme/global-theme";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, Row } from "../../core/platform-controls";
import { TouchKeyboardGeometrySection } from "../touch-keyboard-geometry-section";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { OpenPanelRow } from "../open-panel-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsPreviewBlock } from "../settings-preview-block";
import { SelectRow } from "../select-row";
import type { TouchKeyboardScheme } from "../touch-keyboard-scheme-helpers";

/** 同一方案在 HarmonyOS 键盘上同时画出的 26 键和 9 键两种键盘。其他方案在那里都只有一种键盘。 */
const harmonyLayoutPairs: readonly (readonly [TouchKeyboardScheme, TouchKeyboardScheme])[] = [
  ["quanpin", "nine_key"],
  ["japanese", "japanese_nine_key"],
];

/**
 * HarmonyOS 手机的 中文键盘 行：当前方案的 26 键或 9 键，和 Android 的 `KeyboardOptionsPage` 提供 `touch_keyboard_layout` 的方式一致。选择时像键盘自己的方案选择器那样切到这一对中的另一个键盘，把 `touch_keyboard_layout` 与 `scheme`、`touch_keyboard_schemes.selected` 一起写入（并启用它），因为键盘先读取选中的方案再读布局，只改布局不会让键盘切换。
 */
function HarmonyChineseKeyboardRow({
  selected,
  onSelect,
}: {
  selected: TouchKeyboardScheme;
  onSelect: (scheme: TouchKeyboardScheme) => void;
}) {
  const pair = harmonyLayoutPairs.find((schemes) => schemes.includes(selected));
  const nineKey = pair ? selected === pair[1] : false;
  return (
    <SelectRow
      title="中文键盘"
      aria-label="中文键盘"
      description={pair ? undefined : "当前方案只有一种键盘，在「输入」里换方案"}
      disabled={!pair}
      value={nineKey ? "nine_key" : "twenty_six_key"}
      onChange={(event) => {
        if (pair) onSelect(event.target.value === "nine_key" ? pair[1] : pair[0]);
      }}
    >
      <option value="twenty_six_key">26 键</option>
      <option value="nine_key">9 键</option>
    </SelectRow>
  );
}

/** The 屏幕键盘 page of the settings form. */
export function ScreenKeyboardSettingsPage() {
  const {
    client,
    mobilePlatform,
    iosPlatform,
    macosPlatform,
    harmonyPlatform,
    selectedTouchKeyboardScheme,
    selectHomeScheme,
    host,
    draft,
    setDraft,
    busy,
    page,
    selectPage,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
    previewMobileKeyboardHaptics,
    resetTouchKeyboardSettings,
    beginTouchGeometryDrag,
    updateTouchGeometryDrag,
    endTouchGeometryDrag,
    openPanel,
    keyboardPreviewTheme,
    globalTheme,
    customTouchKeyboardSkin,
    touchKeySpacingTenths,
    touchRowSpacingTenths,
    touchKeyboardHeightAdjustment,
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  // 预览放在它所展示的尺寸控件上方；在预览上拖动调的是同一个间距。启动按钮只在宿主能唤出自己的屏幕键盘面板时才存在。
  // HarmonyOS 手机设计稿把控件放在前面，所以那里的预览排在 布局 分组之后。
  const keyboardPreview = (
    <GroupList title="屏幕键盘">
      {client.openScreenKeyboard && (
        <OpenPanelRow
          title="打开屏幕键盘"
          description="使用鼠标或触控方式输入文字与快捷按键"
          action={() => openPanel(client.openScreenKeyboard)}
          className={`secondary ${settings.openButton}`}
        />
      )}
      <SettingsPreviewBlock aria-label="屏幕键盘预览">
        <div
          aria-label="拖动预览调整键盘间距"
          onPointerDown={beginTouchGeometryDrag}
          onPointerMove={updateTouchGeometryDrag}
          onPointerUp={endTouchGeometryDrag}
          onPointerCancel={endTouchGeometryDrag}
          style={{ touchAction: "none" }}
        >
          <ScreenKeyboardPreview
            theme={keyboardPreviewTheme}
            skin={keyboardThemeId(globalTheme, draft.custom_theme)}
            customDesign={customTouchKeyboardSkin}
            keySpacingTenths={touchKeySpacingTenths}
            rowSpacingTenths={touchRowSpacingTenths}
            heightAdjustment={touchKeyboardHeightAdjustment}
          />
        </div>
      </SettingsPreviewBlock>
    </GroupList>
  );
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "screen-keyboard"} ariaLabel="屏幕键盘">
      {!harmonyPhone && keyboardPreview}
      <TouchKeyboardGeometrySection
        layoutRows={
          harmonyPhone && (
            <HarmonyChineseKeyboardRow
              selected={selectedTouchKeyboardScheme}
              onSelect={selectHomeScheme}
            />
          )
        }
        preview={harmonyPhone && keyboardPreview}
        heightAdjustment={touchKeyboardHeightAdjustment}
        keySpacingTenths={touchKeySpacingTenths}
        rowSpacingTenths={touchRowSpacingTenths}
        touchVoiceShortcut={draft.touch_voice_shortcut ?? false}
        // HarmonyOS 键盘的工具栏已经没有语音按钮（语音在功能面板里），那里也没有任何地方读取 `touch_voice_shortcut`，所以这个开关不会起作用。
        voiceShortcutKind={
          macosPlatform || harmonyPlatform ? "hidden" : iosPlatform ? "last-result" : "start-voice"
        }
        toolbarComponents={Boolean(host?.touch_toolbar_components)}
        toolbar={draft.touch_toolbar}
        tabletFullKeys={mobileKeyboardFeedback?.tabletFullKeys}
        tabletSplitKeyboard={mobileKeyboardFeedback?.tabletSplitKeyboard}
        glideTyping={mobileKeyboardFeedback?.glideTyping}
        swipeSymbols={mobileKeyboardFeedback?.swipeSymbols}
        swipeSymbolsDirection={mobileKeyboardFeedback?.swipeSymbolsDirection}
        numberKeypadOrder={
          mobilePlatform ? (draft.touch_number_keypad_order ?? "phone") : undefined
        }
        tabletFullKeysBusy={mobileKeyboardFeedbackBusy}
        onHeightAdjustmentChange={(touch_keyboard_height_adjustment) =>
          onPreferencesChange({ touch_keyboard_height_adjustment })
        }
        onKeySpacingChange={(touch_key_spacing_tenths) =>
          onPreferencesChange({ touch_key_spacing_tenths })
        }
        onRowSpacingChange={(touch_row_spacing_tenths) =>
          onPreferencesChange({ touch_row_spacing_tenths })
        }
        onTouchVoiceShortcutChange={(touch_voice_shortcut) =>
          onPreferencesChange({ touch_voice_shortcut })
        }
        onNumberKeypadOrderChange={(touch_number_keypad_order) =>
          onPreferencesChange({ touch_number_keypad_order })
        }
        onToolbarChange={(touch_toolbar) => onPreferencesChange({ touch_toolbar })}
        onTabletFullKeysChange={(tabletFullKeys) => {
          if (mobileKeyboardFeedback) {
            void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, tabletFullKeys });
          }
        }}
        onTabletSplitKeyboardChange={(tabletSplitKeyboard) => {
          if (mobileKeyboardFeedback) {
            void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, tabletSplitKeyboard });
          }
        }}
        onGlideTypingChange={(glideTyping) => {
          if (mobileKeyboardFeedback) {
            void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, glideTyping });
          }
        }}
        onSwipeSymbolsChange={(swipeSymbols) => {
          if (mobileKeyboardFeedback) {
            void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, swipeSymbols });
          }
        }}
        onSwipeSymbolsDirectionChange={(swipeSymbolsDirection) => {
          if (mobileKeyboardFeedback) {
            void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, swipeSymbolsDirection });
          }
        }}
        onReset={() => void resetTouchKeyboardSettings()}
      />
      <MobileKeyboardFeedbackSettings
        mobile={mobilePlatform}
        client={client.mobileKeyboardFeedback}
        value={mobileKeyboardFeedback}
        busy={mobileKeyboardFeedbackBusy}
        ios={iosPlatform}
        // 「英文建议」管的是候选，放在输入页的「候选与联想」组。
        showEnglishSuggestions={false}
        onChange={(value) => void saveMobileKeyboardFeedback(value)}
        onPreview={() => void previewMobileKeyboardHaptics()}
      />
      {/* 键盘自己的明暗覆盖与其他按界面的覆盖一起放在「主题 › 高级」下（dc.html）；这一行只是指向那里。全局主题、「我的皮肤」和它的编辑器也都在「主题」页。 */}
      <GroupList title="外观">
        <LinkRow
          title="屏幕键盘外观"
          description="主题与明暗外观在「主题」页的「高级」中设置"
          onClick={() => selectPage("skin")}
        />
      </GroupList>
    </SettingsPageFieldset>
  );
}
