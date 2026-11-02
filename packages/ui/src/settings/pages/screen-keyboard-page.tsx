import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";
import { keyboardThemeId } from "../../theme/global-theme";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow } from "../../core/platform-controls";
import {
  TouchKeyboardGeometrySection,
  type NineKeySwipeDigits,
} from "../touch-keyboard-geometry-section";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { OpenPanelRow } from "../open-panel-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsPreviewBlock } from "../settings-preview-block";
import { SelectRow } from "../select-row";
import type { TouchKeyboardScheme } from "../touch-keyboard-scheme-helpers";
import { supportedInputSchemes } from "../input-scheme-options";
import { touchSplitKeyboardLimits } from "../touch-split-keyboard-rows";
import { defaultTouchKeyboardGeometry } from "../touch-keyboard-geometry-defaults";
import { HandwritingSettingsContent } from "./handwriting-page";
import { screenKeyboardAndHandwritingTitle } from "../settings-page-projections";
import { useSettingsTabs } from "../use-settings-tabs";
import { pluginPreferences } from "../plugin-preferences";
import { useTouchKeySoundPacks } from "../use-touch-key-sound-packs";
import { useEffect, useState } from "react";
import { useMobileSettingsSubview } from "../use-mobile-settings-subview";
import { fullScreenColumn, fullScreenSurface } from "../../core/full-screen-style";
import { SettingsPageHeader } from "../settings-page-header";
import {
  TouchKeyLayoutSettings,
  harmonyTouchRowsForPreferences,
  touchKeyLayoutGroups,
} from "../touch-key-layout-settings";
import {
  keyLayoutEntryDescription,
  nineKeySidebarKindForPreference,
  type NineKeySidebarKind,
} from "../../keyboard/touch-key-layout-styles";
import { SettingsSearchReveal } from "../settings-search-reveal";

/** 同一方案在 HarmonyOS 键盘上画出的几种键盘：全拼有 26/14/18/9 键，双拼有独立的 26/14/18 键，五笔有独立的 26/14 键，日语有 26/9 键。 */
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
  wubiLayout,
  onSelect,
  onShuangpinLayoutChange,
  onWubiLayoutChange,
}: {
  selected: TouchKeyboardScheme;
  shuangpinLayout: "twenty_six_key" | "fourteen_key" | "eighteen_key";
  wubiLayout: "twenty_six_key" | "fourteen_key";
  onSelect: (scheme: TouchKeyboardScheme) => void;
  onShuangpinLayoutChange: (layout: "twenty_six_key" | "fourteen_key" | "eighteen_key") => void;
  onWubiLayoutChange: (layout: "twenty_six_key" | "fourteen_key") => void;
}) {
  // 五笔的 26 / 14 键单独存在 `touch_wubi_layout`，只改它，不动 `scheme`、`wubi_profile` 和启用列表，86 / 98 版共用这一个选择。
  if (selected === "wubi") {
    return (
      <SelectRow
        title="五笔键盘"
        aria-label="五笔键盘"
        value={wubiLayout}
        onChange={(event) => onWubiLayoutChange(event.target.value as typeof wubiLayout)}
      >
        <option value="twenty_six_key">26 键</option>
        <option value="fourteen_key">14 键</option>
      </SelectRow>
    );
  }
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
      detail={group ? undefined : "当前方案只有一种键盘，在「输入」里换方案"}
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

/** 共享偏好 `touch_nine_key_swipe_digits`：缺键和本版本不认识的写法显示「关闭」，与键盘的 `NineKeySwipePolicy.mode` 相同。 */
function nineKeySwipeDigits(value: unknown): NineKeySwipeDigits {
  return value === "up" || value === "down" ? value : "off";
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
    linkTarget,
    clearLinkTarget,
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
  // 「按键布局」子页：HarmonyOS 手机上和语音页的「服务配置」一样压一条历史记录，系统返回手势先关它；离开这页时随之关上。
  const {
    value: keyLayoutSubview,
    open: openKeyLayoutSubview,
    close: closeKeyLayout,
    reset: resetKeyLayout,
  } = useMobileSettingsSubview<boolean>({
    mobile: harmonyPhone,
    stateKey: "keyLayout",
    decode: (state) => (state?.keyLayout === true ? true : null),
    encode: () => ({ page: "screen-keyboard", keyLayout: true }),
    isActiveState: (state) => state?.keyLayout === true,
  });
  useEffect(() => {
    if (page !== "screen-keyboard") resetKeyLayout();
  }, [page, resetKeyLayout]);
  // 版本没有的键盘类型不进子页的分段控件；一种都没有时（只有日文、韩文这类方案的版本）也不画入口。
  const keyLayoutGroups = touchKeyLayoutGroups(supportedInputSchemes(host));
  const keyLayoutOffered =
    harmonyPhone && (keyLayoutGroups.twentySix || keyLayoutGroups.fourteen || keyLayoutGroups.nine);
  // 键盘长按九键左侧符号栏带来的那张表（链接的子目标是它的共享偏好名，与 Android `edit_sidebar_key` 相同）：打开「按键布局」，再由子页打开那张表的编辑页。字母表只在 9 键分段里，版本没有 9 键时停在「按键布局」。子目标认不得或这一页没有「按键布局」时只是取走它，停在「屏幕键盘」页。
  const [sidebarEdit, setSidebarEdit] = useState<{
    kind: NineKeySidebarKind;
    serial: number;
  } | null>(null);
  useEffect(() => {
    if (!linkTarget || linkTarget.page !== "screen-keyboard" || page !== "screen-keyboard") return;
    clearLinkTarget();
    const kind = nineKeySidebarKindForPreference(linkTarget.target);
    if (!kind || !keyLayoutOffered) return;
    if (keyLayoutSubview !== true) openKeyLayoutSubview(true);
    setSidebarEdit({ kind, serial: linkTarget.serial });
  }, [linkTarget, page, keyLayoutOffered, keyLayoutSubview, openKeyLayoutSubview, clearLinkTarget]);
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
          ? harmonyTouchRowsForPreferences(draft, selectedTouchKeyboardScheme)
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
  const body = (
    <>
      {tabList}
      {panel(
        "keyboard",
        <>
          {!harmonyPhone && keyboardPreview}
          <TouchKeyboardGeometrySection
            layoutRows={
              harmonyPhone && (
                <>
                  <HarmonyChineseKeyboardRow
                    selected={selectedTouchKeyboardScheme}
                    shuangpinLayout={draft.touch_shuangpin_layout ?? "twenty_six_key"}
                    wubiLayout={draft.touch_wubi_layout ?? "twenty_six_key"}
                    onSelect={selectHomeScheme}
                    onShuangpinLayoutChange={(touch_shuangpin_layout) =>
                      onPreferencesChange({ touch_shuangpin_layout })
                    }
                    onWubiLayoutChange={(touch_wubi_layout) =>
                      onPreferencesChange({ touch_wubi_layout })
                    }
                  />
                  {keyLayoutOffered && (
                    <LinkRow
                      title="按键布局"
                      description={keyLayoutEntryDescription}
                      onClick={() => openKeyLayoutSubview(true)}
                    />
                  )}
                </>
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
            letterCornerHints={mobileKeyboardFeedback?.letterCornerHints}
            // 只有触屏宿主的 26 键双拼画键位提示；不含双拼的版本（五笔、日文、越南文、藏文等）里不存在双拼键盘，和 Android、iOS 原生设置页一样不列出。缺省为关，与 client-core 的默认值一致。
            shuangpinKeyHints={
              mobilePlatform && supportedInputSchemes(host).includes("shuangpin")
                ? (draft.touch_shuangpin_key_hints ?? false)
                : undefined
            }
            // 全拼 26 键组字保留 ⇧ 的开关：触屏宿主都提供（Android、iOS 原生设置页已有同一行），缺省为开。
            quanpinShift={mobilePlatform ? (draft.touch_quanpin_shift ?? true) : undefined}
            // 大屏单手的侧边数字键盘：只有报告 `touch_one_handed_side_panel` 的宿主（HarmonyOS 平板和可折叠手机）在这里给出，Android 和 iOS 在原生设置页里给。关掉写 off，client-core 存成缺键。
            oneHandedSidePanel={
              host?.touch_one_handed_side_panel
                ? draft.touch_one_handed_side_panel === "number_pad"
                : undefined
            }
            // 分离式键盘：只有报告 `touch_keyboard_split` 的宿主（HarmonyOS 平板和可折叠手机）在这里给出，Android 和 iOS 在原生设置页里给。缺键按 client-core 的默认值读。
            splitKeyboard={
              host?.touch_keyboard_split
                ? {
                    enabled: draft.touch_keyboard_split ?? false,
                    portrait: draft.touch_keyboard_split_portrait ?? false,
                    threshold:
                      draft.touch_keyboard_split_threshold ??
                      touchSplitKeyboardLimits.threshold.fallback,
                    gap: draft.touch_keyboard_split_gap ?? touchSplitKeyboardLimits.gap.fallback,
                  }
                : undefined
            }
            // 「九键滑动输入数字」：触屏宿主、且版本提供全拼九键（`nine_key` 背后是全拼）时列出，桌面和没有拼音九键的版本不画；写共享偏好，不经本机的按键反馈设置。
            nineKeySwipeDigits={
              mobilePlatform && supportedInputSchemes(host).includes("quanpin")
                ? nineKeySwipeDigits(draft.touch_nine_key_swipe_digits)
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
            onQuanpinShiftChange={(touch_quanpin_shift) =>
              onPreferencesChange({ touch_quanpin_shift })
            }
            onOneHandedSidePanelChange={(enabled) =>
              onPreferencesChange({ touch_one_handed_side_panel: enabled ? "number_pad" : "off" })
            }
            onSplitKeyboardChange={(change) =>
              onPreferencesChange({
                ...(change.enabled === undefined ? {} : { touch_keyboard_split: change.enabled }),
                ...(change.portrait === undefined
                  ? {}
                  : { touch_keyboard_split_portrait: change.portrait }),
                ...(change.threshold === undefined
                  ? {}
                  : { touch_keyboard_split_threshold: change.threshold }),
                ...(change.gap === undefined ? {} : { touch_keyboard_split_gap: change.gap }),
              })
            }
            onNineKeySwipeDigitsChange={(touch_nine_key_swipe_digits) =>
              onPreferencesChange({ touch_nine_key_swipe_digits })
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
            onLetterCornerHintsChange={(letterCornerHints) => {
              if (mobileKeyboardFeedback) {
                void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, letterCornerHints });
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
    </>
  );
  // 「按键布局」子页和这页的正文一直都挂着，只是轮流隐藏，与语音页的「服务配置」一样：设置项搜索才扫得到子页里的选项，命中时由 SettingsSearchReveal 打开子页（或关上子页回到正文）。
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "screen-keyboard"}
      ariaLabel={handwritingOnScreenKeyboardPage ? screenKeyboardAndHandwritingTitle : "屏幕键盘"}
    >
      {keyLayoutOffered ? (
        <>
          {/* 两层 div：外层的 hidden 属性管显隐，内层的 flex 排版类不会把它盖掉。 */}
          <SettingsSearchReveal hidden={keyLayoutSubview === true} onReveal={closeKeyLayout}>
            <div className="flex flex-col gap-6">{body}</div>
          </SettingsSearchReveal>
          <SettingsSearchReveal
            label="按键布局"
            hidden={keyLayoutSubview !== true}
            onReveal={() => {
              if (keyLayoutSubview !== true) openKeyLayoutSubview(true);
            }}
          >
            <div
              className={fullScreenSurface}
              role="dialog"
              aria-modal="true"
              aria-label="按键布局"
            >
              <section className={fullScreenColumn}>
                <SettingsPageHeader title="按键布局" onBack={closeKeyLayout} level={2} />
                <TouchKeyLayoutSettings
                  preferences={draft}
                  scheme={selectedTouchKeyboardScheme}
                  groups={keyLayoutGroups}
                  open={keyLayoutSubview === true}
                  mobile={harmonyPhone}
                  editRequest={sidebarEdit}
                  onChange={onPreferencesChange}
                />
              </section>
            </div>
          </SettingsSearchReveal>
        </>
      ) : (
        body
      )}
    </SettingsPageFieldset>
  );
}
