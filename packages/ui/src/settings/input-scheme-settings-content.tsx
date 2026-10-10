import type { EditionInfo, InputScheme, Preferences } from "../index";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { InputModeSection } from "./input-mode-section";
import {
  MacosInputModeEntriesSection,
  type MacosInputModesClient,
} from "./macos-input-mode-entries-section";
import {
  InputSchemeDetailsSection,
  type ShuangpinProfile,
  type WubiProfile,
} from "./input-scheme-details-section";
import {
  baseInputSchemes,
  editionDefaultChineseScheme,
  fallbackChineseScheme,
  isChineseScheme,
  singleEditionScheme,
} from "./input-scheme-options";
import {
  InputSchemeSelectorSection,
  type InputSchemeSelectorValue,
} from "./input-scheme-selector-section";
import {
  touchKeyboardSchemeInputScheme,
  touchKeyboardSchemeOptions,
  wubiProfileTitle,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";
import { TouchKeyboardSchemesSection } from "./touch-keyboard-schemes-section";
import { LanguageCard } from "./language-card";
import { WubiSection } from "./wubi-section";
import { ResourcePackRow, resourcePackForScheme, type ResourcePacks } from "./resource-packs";

export interface InputSchemeSettingsContentProps {
  preferences: Preferences;
  hasTouchKeyboardSchemes: boolean;
  touchKeyboardSchemes: { enabled: readonly TouchKeyboardScheme[] };
  selectedTouchKeyboardScheme: TouchKeyboardScheme;
  macos: boolean;
  /** The schemes the host offers (`supportedInputSchemes(host)`); the others are shown disabled. Defaults to the five every host offers. */
  inputSchemes?: readonly InputScheme[];
  /** 运行中的版本（`HostCapabilities.edition`），不是 full 时才有：本版本没有的方案和触屏键盘不列出，只有一个方案时隐藏方案选择。 */
  edition?: EditionInfo;
  /** 「输入时显示双拼键位提示」的当前值，宿主不画双拼键位图时不传；改动写进共享偏好 `shuangpin_keymap_hint`。 */
  shuangpinKeymapHint?: boolean;
  /** macOS 的输入法列表；有它时在方案组末尾显示「菜单栏入口」。 */
  macosInputModes?: MacosInputModesClient;
  onError?: (message: string) => void;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  onSelectTouchKeyboardScheme: (scheme: TouchKeyboardScheme) => void;
  onToggleTouchKeyboardScheme: (scheme: TouchKeyboardScheme, enabled: boolean) => void;
  /** 宿主提供按需资源包时传入（目前只有 macOS）：选用日文、粤拼、注音或笔画会照常保存方案并开始下载对应词库，下载完成前运行时按缺少词库回退。 */
  resourcePacks?: ResourcePacks;
  /** HarmonyOS 手机把触屏方案画成「语言与方案」卡片，而不是每个方案一个开关，余下的方案选项收在其下方的「更多选项」折叠区里。仅在 `hasTouchKeyboardSchemes` 时生效。 */
  languageCard?: boolean;
  /** 把某个触屏方案设为当前方案，若它处于关闭状态则先启用；语言卡片的面板使用它。默认为 `onSelectTouchKeyboardScheme`。 */
  onEnableAndSelectTouchKeyboardScheme?: (scheme: TouchKeyboardScheme) => void;
}

/** Shared input mode, scheme selector, scheme details, and Wubi composition for settings hosts. */
export function InputSchemeSettingsContent({
  preferences,
  hasTouchKeyboardSchemes,
  touchKeyboardSchemes,
  selectedTouchKeyboardScheme,
  macos,
  inputSchemes = baseInputSchemes,
  edition,
  shuangpinKeymapHint,
  macosInputModes,
  onError = () => {},
  onPreferencesChange,
  onSelectTouchKeyboardScheme,
  onToggleTouchKeyboardScheme,
  resourcePacks,
  languageCard = false,
  onEnableAndSelectTouchKeyboardScheme = onSelectTouchKeyboardScheme,
}: InputSchemeSettingsContentProps) {
  // 方案改动立即写入草稿（随自动保存生效），需要词库的方案再在后台下载，不等下载完成。
  const onSchemeChange = (patch: Partial<Preferences>) => {
    onPreferencesChange(patch);
    const pack = resourcePackForScheme(patch.scheme);
    if (pack) resourcePacks?.ensure(pack);
  };
  const schemePack = resourcePackForScheme(preferences.scheme);
  const chineseSchemes = isChineseScheme(preferences.scheme);
  const defaultScheme = editionDefaultChineseScheme(edition);
  // 文档里的方案不属于本版本时（例如从别的版本带来的旧文档），输入模式一行可能只剩「中文」而隐藏，所以方案选择照常显示，选中的是 host-api 实际运行的方案，点一下就能改回本版本的方案。
  const outsideEdition =
    edition !== undefined && !edition.input_schemes.includes(preferences.scheme);
  const selectorValue: InputSchemeSelectorValue = isChineseScheme(preferences.scheme)
    ? preferences.scheme
    : outsideEdition
      ? fallbackChineseScheme(preferences.last_chinese_scheme, inputSchemes, defaultScheme)
      : "quanpin";
  // 只有五笔一个方案的版本始终显示五笔的设置。
  const wubiEdition = singleEditionScheme(edition) === "wubi";
  // 粤拼、注音、越南语、藏文和笔画的触屏键盘输入各自的方案，所以只在宿主提供该方案时出现（粤拼、注音和笔画还需要装好词库）。
  // 五笔键盘只有一个，标题跟随当前的五笔版本。
  // 不是 full 的版本还要去掉本版本没有的方案对应的键盘；手写不属于任何方案，由版本表的 `features.handwriting`（`EditionInfo.handwriting`）决定：手写识别器只认汉字，只在提供中文方案的版本里保留（日文、越南文、藏文版没有），与 client-core 的 `Edition::offers_touch_scheme` 一致。
  const touchOptions = touchKeyboardSchemeOptions
    .filter(
      ([scheme]) =>
        (scheme !== "cantonese" &&
          scheme !== "zhuyin" &&
          scheme !== "vietnamese" &&
          scheme !== "tibetan" &&
          scheme !== "stroke" &&
          scheme !== "zhuyin_nine_key") ||
        inputSchemes.includes(scheme === "zhuyin_nine_key" ? "zhuyin" : scheme),
    )
    .filter(([scheme]) => {
      if (!edition) return true;
      const input = touchKeyboardSchemeInputScheme(scheme);
      return input === null ? edition.handwriting : edition.input_schemes.includes(input);
    })
    .map(([scheme, title]): [TouchKeyboardScheme, string] => [
      scheme,
      scheme === "wubi" ? wubiProfileTitle(preferences.wubi_profile) : title,
    ]);
  const touchHasWubi = hasTouchKeyboardSchemes && touchKeyboardSchemes.enabled.includes("wubi");
  const showWubiSection = touchHasWubi || preferences.scheme === "wubi" || wubiEdition;
  const wubiSection = showWubiSection && (
    <WubiSection
      preferences={preferences}
      mixedPinyinDefault={edition?.wubi_mixed_pinyin_default}
      onChange={onPreferencesChange}
    />
  );
  const schemePackRow = resourcePacks && schemePack && (
    <ResourcePackRow packs={resourcePacks} id={schemePack} />
  );
  if (hasTouchKeyboardSchemes && languageCard) {
    const vietnamese =
      touchKeyboardSchemes.enabled.includes("vietnamese") &&
      touchOptions.some(([scheme]) => scheme === "vietnamese");
    return (
      <GroupList title="语言与方案">
        <LanguageCard
          available={touchOptions.map(([scheme]) => scheme)}
          enabled={touchKeyboardSchemes.enabled}
          selected={selectedTouchKeyboardScheme}
          wubiProfile={preferences.wubi_profile}
          onSelect={onEnableAndSelectTouchKeyboardScheme}
          onToggle={onToggleTouchKeyboardScheme}
          onWubiProfileChange={(wubi_profile) => onPreferencesChange({ wubi_profile })}
        />
        {(schemePackRow || wubiSection || vietnamese) && (
          <MoreOptions>
            {schemePackRow}
            {wubiSection}
            {/* 越南语是唯一有自己选项（输入法和声调位置）的触屏语言。详情区在触屏宿主上隐藏其各行，因为那里原先由方案开关列表代替选择器，所以这里按没有触屏列表时越南语方案的画法单独绘制；其他方案的各行保持隐藏。 */}
            {vietnamese && (
              <InputSchemeDetailsSection
                scheme="vietnamese"
                shuangpinProfile={preferences.shuangpin_profile}
                wubiProfile={preferences.wubi_profile}
                macos={false}
                hasTouchKeyboardSchemes={false}
                onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
                  onPreferencesChange({ shuangpin_profile })
                }
                vietnamese={preferences.vietnamese}
                onVietnameseChange={(next) => onPreferencesChange({ vietnamese: next })}
              />
            )}
          </MoreOptions>
        )}
      </GroupList>
    );
  }
  return (
    <GroupList title="方案">
      <InputModeSection
        scheme={preferences.scheme}
        lastChineseScheme={preferences.last_chinese_scheme}
        supportedSchemes={inputSchemes}
        editionSchemes={edition?.input_schemes}
        defaultScheme={defaultScheme}
        hidden={hasTouchKeyboardSchemes}
        onChange={onSchemeChange}
      />
      {hasTouchKeyboardSchemes && (
        <TouchKeyboardSchemesSection
          options={touchOptions}
          enabled={touchKeyboardSchemes.enabled}
          selected={selectedTouchKeyboardScheme}
          onSelect={(scheme) => onSelectTouchKeyboardScheme(scheme as TouchKeyboardScheme)}
          onToggle={(scheme, enabled) =>
            onToggleTouchKeyboardScheme(scheme as TouchKeyboardScheme, enabled)
          }
        />
      )}
      <InputSchemeSelectorSection
        hidden={hasTouchKeyboardSchemes || (!chineseSchemes && !outsideEdition)}
        value={selectorValue}
        supportedSchemes={inputSchemes}
        editionSchemes={edition?.input_schemes}
        defaultScheme={defaultScheme}
        lastChineseScheme={preferences.last_chinese_scheme}
        onChange={(scheme: InputSchemeSelectorValue) =>
          onSchemeChange({ scheme, last_chinese_scheme: scheme })
        }
      />
      <InputSchemeDetailsSection
        scheme={preferences.scheme}
        shuangpinProfile={preferences.shuangpin_profile}
        wubiProfile={preferences.wubi_profile}
        macos={macos}
        hasTouchKeyboardSchemes={hasTouchKeyboardSchemes}
        touchKeyboardHasWubi={touchHasWubi}
        shuangpinKeymapHint={shuangpinKeymapHint}
        onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
          onPreferencesChange({ shuangpin_profile })
        }
        onWubiProfileChange={(wubi_profile: WubiProfile) => onPreferencesChange({ wubi_profile })}
        onShuangpinKeymapHintChange={(shuangpin_keymap_hint) =>
          onPreferencesChange({ shuangpin_keymap_hint })
        }
        vietnamese={preferences.vietnamese}
        onVietnameseChange={(vietnamese) => onPreferencesChange({ vietnamese })}
      />
      {schemePackRow}
      {wubiSection}
      {macos && (
        <MacosInputModeEntriesSection
          client={macosInputModes}
          scheme={preferences.scheme}
          inputSchemes={inputSchemes}
          edition={edition}
          onError={onError}
        />
      )}
    </GroupList>
  );
}
