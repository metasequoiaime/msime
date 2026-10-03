import type { InputScheme, Preferences } from "../index";
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
import { baseInputSchemes, isChineseScheme } from "./input-scheme-options";
import {
  InputSchemeSelectorSection,
  type InputSchemeSelectorValue,
} from "./input-scheme-selector-section";
import {
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
  /** 宿主提供按需资源包时传入（目前只有 macOS）：选用日文、粤拼、注音或笔画会照常保存方案并开始下载对应词库，下载完成前运行时按缺少词库回退。 */
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
  // The Cantonese, Zhuyin, Vietnamese and Stroke touch keyboards type their own input scheme, so they are offered only where the host offers that scheme (Cantonese, Zhuyin and Stroke also need their installed dictionary).
  // 五笔键盘只有一个，标题跟随当前的五笔版本。
  const touchOptions = touchKeyboardSchemeOptions
    .filter(
      ([scheme]) =>
        (scheme !== "cantonese" &&
          scheme !== "zhuyin" &&
          scheme !== "vietnamese" &&
          scheme !== "stroke") ||
        inputSchemes.includes(scheme),
    )
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
        hidden={hasTouchKeyboardSchemes || !chineseSchemes}
        value={isChineseScheme(preferences.scheme) ? preferences.scheme : "quanpin"}
        supportedSchemes={inputSchemes}
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
        preferences.scheme === "wubi") && (
        <WubiSection
          preferences={preferences}
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
