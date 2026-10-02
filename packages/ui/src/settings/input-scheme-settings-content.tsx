import type { InputScheme, Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
import { InputModeSection } from "./input-mode-section";
import {
  MacosInputModeEntriesSection,
  type MacosInputModesClient,
} from "./macos-input-mode-entries-section";
import { InputSchemeDetailsSection, type ShuangpinProfile } from "./input-scheme-details-section";
import { baseInputSchemes, isChineseScheme } from "./input-scheme-options";
import {
  InputSchemeSelectorSection,
  type InputSchemeSelectorValue,
} from "./input-scheme-selector-section";
import {
  touchKeyboardSchemeOptions,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";
import { TouchKeyboardSchemesSection } from "./touch-keyboard-schemes-section";
import { WubiSection } from "./wubi-section";

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
}: InputSchemeSettingsContentProps) {
  const chineseSchemes = isChineseScheme(preferences.scheme);
  // The Cantonese, Zhuyin and Vietnamese touch keyboards type their own input scheme, so they are offered only where the host offers that scheme (Cantonese and Zhuyin also need their installed dictionary).
  const touchOptions = touchKeyboardSchemeOptions.filter(
    ([scheme]) =>
      (scheme !== "cantonese" && scheme !== "zhuyin" && scheme !== "vietnamese") ||
      inputSchemes.includes(scheme),
  );
  return (
    <GroupList title="方案">
      <InputModeSection
        scheme={preferences.scheme}
        lastChineseScheme={preferences.last_chinese_scheme}
        supportedSchemes={inputSchemes}
        hidden={hasTouchKeyboardSchemes}
        onChange={onPreferencesChange}
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
          onPreferencesChange({ scheme, last_chinese_scheme: scheme })
        }
      />
      <InputSchemeDetailsSection
        scheme={preferences.scheme}
        shuangpinProfile={preferences.shuangpin_profile}
        macos={macos}
        hasTouchKeyboardSchemes={hasTouchKeyboardSchemes}
        macosShuangpinKeymap={macosShuangpinKeymap}
        onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
          onPreferencesChange({ shuangpin_profile })
        }
        onMacosShuangpinKeymapChange={onMacosShuangpinKeymapChange}
        vietnamese={preferences.vietnamese}
        onVietnameseChange={(vietnamese) => onPreferencesChange({ vietnamese })}
      />
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
