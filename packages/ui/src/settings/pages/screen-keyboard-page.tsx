import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";
import { keyboardThemeId } from "../../theme/global-theme";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, Row } from "../../core/platform-controls";
import { TouchKeyboardGeometrySection } from "../touch-keyboard-geometry-section";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { OpenPanelButton } from "../open-panel-button";

/** The 屏幕键盘 page of the settings form. */
export function ScreenKeyboardSettingsPage() {
  const {
    client,
    mobilePlatform,
    iosPlatform,
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
  return (
    <fieldset disabled={busy} hidden={page !== "screen-keyboard"} aria-label="屏幕键盘">
      <div className={settings.groups}>
        {/* 预览放在它所展示的尺寸控件上方；在预览上拖动调的是同一个间距。启动按钮只在宿主能唤出自己的屏幕键盘面板时才存在。 */}
        <GroupList title="屏幕键盘">
          {client.openScreenKeyboard && (
            <Row title="打开屏幕键盘" description="使用鼠标或触控方式输入文字与快捷按键">
              <OpenPanelButton
                action={() => openPanel(client.openScreenKeyboard)}
                className={`secondary ${settings.openButton}`}
              />
            </Row>
          )}
          <div className={settings.groupPreview} aria-label="屏幕键盘预览">
            <div className={settings.panelPreviewLabel}>预览</div>
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
          </div>
        </GroupList>
        <TouchKeyboardGeometrySection
          heightAdjustment={touchKeyboardHeightAdjustment}
          keySpacingTenths={touchKeySpacingTenths}
          rowSpacingTenths={touchRowSpacingTenths}
          touchVoiceShortcut={draft.touch_voice_shortcut ?? false}
          toolbarComponents={Boolean(host?.touch_toolbar_components)}
          toolbar={draft.touch_toolbar}
          tabletFullKeys={mobileKeyboardFeedback?.tabletFullKeys}
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
          onToolbarChange={(touch_toolbar) => onPreferencesChange({ touch_toolbar })}
          onTabletFullKeysChange={(tabletFullKeys) => {
            if (mobileKeyboardFeedback) {
              void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, tabletFullKeys });
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
      </div>
    </fieldset>
  );
}
