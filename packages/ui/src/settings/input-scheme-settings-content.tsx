import type { EditionInfo, InputScheme, Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
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
  macosShuangpinKeymap?: boolean;
  macosWubiAutoCommitUnique?: boolean;
  /** macOS 的输入法列表；有它时在方案组末尾显示「菜单栏入口」。 */
  macosInputModes?: MacosInputModesClient;
  onError?: (message: string) => void;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  onSelectTouchKeyboardScheme: (scheme: TouchKeyboardScheme) => void;
  onToggleTouchKeyboardScheme: (scheme: TouchKeyboardScheme, enabled: boolean) => void;
  onMacosShuangpinKeymapChange: (enabled: boolean) => void;
  onMacosWubiAutoCommitUniqueChange: (enabled: boolean) => void;
  /** 宿主提供按需资源包时传入（目前只有 macOS）：选用日文、粤拼或注音会照常保存方案并开始下载对应词库，下载完成前运行时按缺少词库回退。 */
  resourcePacks?: ResourcePacks;
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
  macosShuangpinKeymap,
  macosWubiAutoCommitUnique,
  macosInputModes,
  onError = () => {},
  onPreferencesChange,
  onSelectTouchKeyboardScheme,
  onToggleTouchKeyboardScheme,
  onMacosShuangpinKeymapChange,
  onMacosWubiAutoCommitUniqueChange,
  resourcePacks,
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
  // The Cantonese, Zhuyin and Vietnamese touch keyboards type their own input scheme, so they are offered only where the host offers that scheme (Cantonese and Zhuyin also need their installed dictionary).
  // 五笔键盘只有一个，标题跟随当前的五笔版本。
  // 不是 full 的版本还要去掉本版本没有的方案对应的键盘；手写不属于任何方案，每个版本都保留。
  const touchOptions = touchKeyboardSchemeOptions
    .filter(
      ([scheme]) =>
        (scheme !== "cantonese" && scheme !== "zhuyin" && scheme !== "vietnamese") ||
        inputSchemes.includes(scheme),
    )
    .filter(([scheme]) => {
      const input = touchKeyboardSchemeInputScheme(scheme);
      return !edition || input === null || edition.input_schemes.includes(input);
    })
    .map(([scheme, title]): [TouchKeyboardScheme, string] => [
      scheme,
      scheme === "wubi" ? wubiProfileTitle(preferences.wubi_profile) : title,
    ]);
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
        touchKeyboardHasWubi={touchKeyboardSchemes.enabled.includes("wubi")}
        macosShuangpinKeymap={macosShuangpinKeymap}
        onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
          onPreferencesChange({ shuangpin_profile })
        }
        onWubiProfileChange={(wubi_profile: WubiProfile) => onPreferencesChange({ wubi_profile })}
        onMacosShuangpinKeymapChange={onMacosShuangpinKeymapChange}
        vietnamese={preferences.vietnamese}
        onVietnameseChange={(vietnamese) => onPreferencesChange({ vietnamese })}
      />
      {resourcePacks && schemePack && <ResourcePackRow packs={resourcePacks} id={schemePack} />}
      {((hasTouchKeyboardSchemes && touchKeyboardSchemes.enabled.includes("wubi")) ||
        preferences.scheme === "wubi" ||
        wubiEdition) && (
        <WubiSection
          preferences={preferences}
          mixedPinyinDefault={edition?.wubi_mixed_pinyin_default}
          autoCommitUnique={macos ? macosWubiAutoCommitUnique : undefined}
          onChange={onPreferencesChange}
          onAutoCommitUniqueChange={onMacosWubiAutoCommitUniqueChange}
        />
      )}
      {macos && (
        <MacosInputModeEntriesSection
          client={macosInputModes}
          scheme={preferences.scheme}
          inputSchemes={inputSchemes}
          onError={onError}
        />
      )}
    </GroupList>
  );
}
