import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";
import { keyboardThemeId } from "../../theme/global-theme";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import { harmonyTouchKeyboardRows, harmonyTouchLayout } from "../../keyboard/keyboard-layouts";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow } from "../../core/platform-controls";
import { TouchKeyboardGeometrySection } from "../touch-keyboard-geometry-section";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { OpenPanelRow } from "../open-panel-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsPreviewBlock } from "../settings-preview-block";
import { SelectRow } from "../select-row";
import type { TouchKeyboardScheme } from "../touch-keyboard-scheme-helpers";
import { supportedInputSchemes } from "../input-scheme-options";
import { defaultTouchKeyboardGeometry } from "../touch-keyboard-geometry-defaults";
import { HandwritingSettingsContent } from "./handwriting-page";
import { screenKeyboardAndHandwritingTitle } from "../settings-page-projections";
import { useSettingsTabs } from "../use-settings-tabs";
import { pluginPreferences } from "../plugin-preferences";
import { useTouchKeySoundPacks } from "../use-touch-key-sound-packs";

/** 同一方案在 HarmonyOS 键盘上画出的几种键盘：全拼有 26/14/18/9 键，双拼有独立的 26/14/18 键，日语有 26/9 键。 */
type HarmonyKeyboardLayout = "twenty_six_key" | "fourteen_key" | "eighteen_key" | "nine_key";
const harmonyLayoutGroups: readonly (readonly (readonly [
  HarmonyKeyboardLayout,
  TouchKeyboardScheme,
])[])[] = [
  [
    ["twenty_six_key", "quanpin"],
    ["fourteen_key", "fourteen_key"],
    ["eighteen_key", "eighteen_key"],
    ["nine_key", "nine_key"],
  ],
  [
    ["twenty_six_key", "japanese"],
    ["nine_key", "japanese_nine_key"],
  ],
];

const harmonyLayoutTitles: Record<HarmonyKeyboardLayout, string> = {
  twenty_six_key: "26 键",
  fourteen_key: "14 键",
  eighteen_key: "18 键",
  nine_key: "9 键",
};

/**
 * HarmonyOS 手机的 中文键盘 行：当前方案的 26 键、14 键或 9 键，和 Android 的 `KeyboardOptionsPage` 提供 `touch_keyboard_layout` 的方式一致。选择时像键盘自己的方案选择器那样切到这一组中的另一个键盘，把 `touch_keyboard_layout` 与 `scheme`、`touch_keyboard_schemes.selected` 一起写入（并启用它），因为键盘先读取选中的方案再读布局，只改布局不会让键盘切换。
 */
function HarmonyChineseKeyboardRow({
  selected,
  shuangpinLayout,
  onSelect,
  onShuangpinLayoutChange,
}: {
  selected: TouchKeyboardScheme;
  shuangpinLayout: "twenty_six_key" | "fourteen_key" | "eighteen_key";
  onSelect: (scheme: TouchKeyboardScheme) => void;
  onShuangpinLayoutChange: (layout: "twenty_six_key" | "fourteen_key" | "eighteen_key") => void;
}) {
  const shuangpin =
    selected === "xiaohe" ||
    selected === "ziranma" ||
    selected === "microsoft" ||
    selected === "shoudao";
  if (shuangpin) {
    return (
      <SelectRow
        title="中文键盘"
        aria-label="中文键盘"
        value={shuangpinLayout}
        onChange={(event) => onShuangpinLayoutChange(event.target.value as typeof shuangpinLayout)}
      >
        <option value="twenty_six_key">26 键</option>
        <option value="fourteen_key">14 键</option>
        <option value="eighteen_key">18 键</option>
      </SelectRow>
    );
  }
  const group = harmonyLayoutGroups.find((layouts) =>
    layouts.some(([, scheme]) => scheme === selected),
  );
  const current = group?.find(([, scheme]) => scheme === selected)?.[0] ?? "twenty_six_key";
  return (
    <SelectRow
      title="中文键盘"
      aria-label="中文键盘"
      description={group ? undefined : "当前方案只有一种键盘，在「输入」里换方案"}
      disabled={!group}
      value={current}
      onChange={(event) => {
        const scheme = group?.find(([layout]) => layout === event.target.value)?.[1];
        if (scheme) onSelect(scheme);
      }}
    >
      {(group ?? harmonyLayoutGroups[0]).map(([layout]) => (
        <option key={layout} value={layout}>
          {harmonyLayoutTitles[layout]}
        </option>
      ))}
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
    showTouchKeyboardHeight,
    showTouchKeyboardSpacing,
    handwritingOnScreenKeyboardPage,
    introTabs,
    selectIntroTab,
    surfaceThemesFollow,
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  // 只有触屏键盘按所选包发按键音的宿主（HarmonyOS 手机和平板）才读目录、给「音效」行；2in1 和其他宿主在插件页或原生设置里选包，这里不变。
  const keySoundPacks = useTouchKeySoundPacks(
    host?.touch_key_sound_packs ? client.plugins?.catalog : undefined,
    page === "screen-keyboard",
  );
  const plugins = pluginPreferences(draft);
  const keySoundPack =
    keySoundPacks && keySoundPacks.length > 0
      ? {
          selected: plugins.key_sound.pack,
          packs: keySoundPacks,
          onSelect: (pack: string) => {
            onPreferencesChange({
              plugins: { ...plugins, key_sound: { ...plugins.key_sound, pack } },
            });
            // 试听只是顺带的：播不出来不影响已经写入的选择，所以不报错。
            void client.mobileKeyboardFeedback
              ?.previewKeySound?.(pack, plugins.key_sound.volume)
              .catch(() => undefined);
          },
        }
      : undefined;
  // 宿主不按存下的高度或间距画键盘时（macOS），预览画键盘实际的样子：默认高度和默认间距，不画存下的值。
  const previewKeySpacingTenths = showTouchKeyboardSpacing
    ? touchKeySpacingTenths
    : defaultTouchKeyboardGeometry.keySpacingTenths;
  const previewRowSpacingTenths = showTouchKeyboardSpacing
    ? touchRowSpacingTenths
    : defaultTouchKeyboardGeometry.rowSpacingTenths;
  const previewHeightAdjustment = showTouchKeyboardHeight
    ? touchKeyboardHeightAdjustment
    : defaultTouchKeyboardGeometry.heightAdjustment;
  const preview = (
    <ScreenKeyboardPreview
      theme={keyboardPreviewTheme}
      skin={keyboardThemeId(globalTheme, draft.custom_theme)}
      customDesign={customTouchKeyboardSkin}
      keySpacingTenths={previewKeySpacingTenths}
      rowSpacingTenths={previewRowSpacingTenths}
      heightAdjustment={previewHeightAdjustment}
      phone={harmonyPhone}
      touchRows={
        harmonyPhone
          ? harmonyTouchKeyboardRows[
              harmonyTouchLayout(
                selectedTouchKeyboardScheme,
                draft.touch_shuangpin_layout ?? "twenty_six_key",
              )
            ]
          : undefined
      }
    />
  );
  // 在预览上拖动调的是同一个间距；间距不可调的宿主上预览只是静态的样子。
  const draggablePreview = showTouchKeyboardSpacing ? (
    <div
      aria-label="拖动预览调整键盘间距"
      onPointerDown={beginTouchGeometryDrag}
      onPointerMove={updateTouchGeometryDrag}
      onPointerUp={endTouchGeometryDrag}
      onPointerCancel={endTouchGeometryDrag}
      style={{ touchAction: "none" }}
    >
      {preview}
    </div>
  ) : (
    preview
  );
  // HarmonyOS 手机上预览是「尺寸」卡片的第一行，下面就是它调的滑块，底下一行说明可以直接拖。
  const phonePreview = (
    <div className={settings.phoneKeyboardPreview} role="group" aria-label="屏幕键盘预览">
      {draggablePreview}
      <p className={settings.phoneKeyboardPreviewHint}>在预览上左右拖动调键距，上下拖动调行距</p>
    </div>
  );
  // 其他宿主的预览放在它所展示的尺寸控件上方。启动按钮只在宿主能唤出自己的屏幕键盘面板时才存在。
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
      <SettingsPreviewBlock aria-label="屏幕键盘预览">{draggablePreview}</SettingsPreviewBlock>
    </GroupList>
  );
  const { tabList, panel } = useSettingsTabs(
    [
      { id: "keyboard", title: "键盘" },
      { id: "handwriting", title: "手写" },
    ] as const,
    "键盘手写分类",
    handwritingOnScreenKeyboardPage,
    introTabs["screen-keyboard"],
    (tab) => selectIntroTab("screen-keyboard", tab),
  );
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "screen-keyboard"}
      ariaLabel={handwritingOnScreenKeyboardPage ? screenKeyboardAndHandwritingTitle : "屏幕键盘"}
    >
      {tabList}
      {panel(
        "keyboard",
        <>
          {!harmonyPhone && keyboardPreview}
          <TouchKeyboardGeometrySection
            layoutRows={
              harmonyPhone && (
                <HarmonyChineseKeyboardRow
                  selected={selectedTouchKeyboardScheme}
                  shuangpinLayout={draft.touch_shuangpin_layout ?? "twenty_six_key"}
                  onSelect={selectHomeScheme}
                  onShuangpinLayoutChange={(touch_shuangpin_layout) =>
                    onPreferencesChange({ touch_shuangpin_layout })
                  }
                />
              )
            }
            preview={harmonyPhone && phonePreview}
            heightAdjustment={touchKeyboardHeightAdjustment}
            keySpacingTenths={touchKeySpacingTenths}
            rowSpacingTenths={touchRowSpacingTenths}
            showHeight={showTouchKeyboardHeight}
            showSpacing={showTouchKeyboardSpacing}
            touchVoiceShortcut={draft.touch_voice_shortcut ?? false}
            // HarmonyOS 键盘的工具栏已经没有语音按钮（语音在功能面板里），那里也没有任何地方读取 `touch_voice_shortcut`，所以这个开关不会起作用。
            voiceShortcutKind={
              macosPlatform || harmonyPlatform
                ? "hidden"
                : iosPlatform
                  ? "last-result"
                  : "start-voice"
            }
            toolbarComponents={Boolean(host?.touch_toolbar_components)}
            toolbar={draft.touch_toolbar}
            // 只有报告 `touch_toolbar_tools` 的宿主（HarmonyOS 手机）给出这组开关，其他宿主的页面不变。
            harmonyToolbar={host?.touch_toolbar_tools ? (draft.harmony_toolbar ?? {}) : undefined}
            tabletFullKeys={mobileKeyboardFeedback?.tabletFullKeys}
            tabletSplitKeyboard={mobileKeyboardFeedback?.tabletSplitKeyboard}
            glideTyping={mobileKeyboardFeedback?.glideTyping}
            swipeSymbols={mobileKeyboardFeedback?.swipeSymbols}
            swipeSymbolsDirection={mobileKeyboardFeedback?.swipeSymbolsDirection}
            // 只有触屏宿主的 26 键双拼画键位提示；不含双拼的版本（五笔、日文、越南文、藏文等）里不存在双拼键盘，和 Android、iOS 原生设置页一样不列出。缺省为关，与 client-core 的默认值一致。
            shuangpinKeyHints={
              mobilePlatform && supportedInputSchemes(host).includes("shuangpin")
                ? (draft.touch_shuangpin_key_hints ?? false)
                : undefined
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
            onShuangpinKeyHintsChange={(touch_shuangpin_key_hints) =>
              onPreferencesChange({ touch_shuangpin_key_hints })
            }
            onToolbarChange={(touch_toolbar) => onPreferencesChange({ touch_toolbar })}
            onHarmonyToolbarChange={(harmony_toolbar) => onPreferencesChange({ harmony_toolbar })}
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
                void saveMobileKeyboardFeedback({
                  ...mobileKeyboardFeedback,
                  swipeSymbolsDirection,
                });
              }
            }}
            onReset={(subjects) => void resetTouchKeyboardSettings(subjects)}
          />
          <MobileKeyboardFeedbackSettings
            mobile={mobilePlatform}
            client={client.mobileKeyboardFeedback}
            value={mobileKeyboardFeedback}
            busy={mobileKeyboardFeedbackBusy}
            ios={iosPlatform}
            // 「英文建议」管的是候选，放在输入页的「候选与联想」组。
            showEnglishSuggestions={false}
            keySoundPack={keySoundPack}
            onChange={(value) => void saveMobileKeyboardFeedback(value)}
            onPreview={() => void previewMobileKeyboardHaptics()}
          />
          {/* 键盘自己的明暗覆盖与其他按界面的覆盖一起放在「主题 › 高级」下（dc.html）；这一行只是指向那里。全局主题、「我的皮肤」和它的编辑器也都在「主题」页。各界面一律跟随颜色模式的宿主（`surfaceThemesFollow`，macOS、Windows、Linux 和 HarmonyOS）没有「高级」组，键盘跟随「主题」页的颜色模式和主题，描述照实说。HarmonyOS 手机的皮肤页只有网格、没有「高级」，这一行指向的东西不存在，所以不画。 */}
          {!harmonyPhone && (
            <GroupList title="外观">
              <LinkRow
                title="屏幕键盘外观"
                description={
                  surfaceThemesFollow
                    ? "屏幕键盘跟随「主题」页的颜色模式和主题"
                    : "主题与明暗外观在「主题」页的「高级」中设置"
                }
                onClick={() => selectPage("skin")}
              />
            </GroupList>
          )}
        </>,
      )}
      {handwritingOnScreenKeyboardPage && panel("handwriting", <HandwritingSettingsContent />)}
    </SettingsPageFieldset>
  );
}
