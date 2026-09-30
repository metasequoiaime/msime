import type { Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
import { InputModeSection } from "./input-mode-section";
import { InputSchemeDetailsSection, type ShuangpinProfile } from "./input-scheme-details-section";
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
  grouped?: boolean;
  hasTouchKeyboardSchemes: boolean;
  touchKeyboardSchemes: { enabled: readonly TouchKeyboardScheme[] };
  selectedTouchKeyboardScheme: TouchKeyboardScheme;
  macos: boolean;
  macosShuangpinKeymap?: boolean;
  macosWubiAutoCommitUnique?: boolean;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  onSelectTouchKeyboardScheme: (scheme: TouchKeyboardScheme) => void;
  onToggleTouchKeyboardScheme: (scheme: TouchKeyboardScheme, enabled: boolean) => void;
  onMacosShuangpinKeymapChange: (enabled: boolean) => void;
  onMacosWubiAutoCommitUniqueChange: (enabled: boolean) => void;
}

/** Shared input mode, scheme selector, scheme details, and Wubi composition for settings hosts. */
export function InputSchemeSettingsContent({
  preferences,
  grouped = false,
  hasTouchKeyboardSchemes,
  touchKeyboardSchemes,
  selectedTouchKeyboardScheme,
  macos,
  macosShuangpinKeymap,
  macosWubiAutoCommitUnique,
  onPreferencesChange,
  onSelectTouchKeyboardScheme,
  onToggleTouchKeyboardScheme,
  onMacosShuangpinKeymapChange,
  onMacosWubiAutoCommitUniqueChange,
}: InputSchemeSettingsContentProps) {
  const chineseSchemes = preferences.scheme !== "japanese";
  const content = (
    <>
      {!grouped && !hasTouchKeyboardSchemes && (
        <InputModeSection
          scheme={preferences.scheme}
          lastChineseScheme={preferences.last_chinese_scheme}
          onChange={onPreferencesChange}
        />
      )}
      {grouped && (
        <InputModeSection
          scheme={preferences.scheme}
          lastChineseScheme={preferences.last_chinese_scheme}
          hidden={hasTouchKeyboardSchemes}
          onChange={onPreferencesChange}
        />
      )}
      {hasTouchKeyboardSchemes && (
        <TouchKeyboardSchemesSection
          options={touchKeyboardSchemeOptions}
          enabled={touchKeyboardSchemes.enabled}
          selected={selectedTouchKeyboardScheme}
          onSelect={(scheme) => onSelectTouchKeyboardScheme(scheme as TouchKeyboardScheme)}
          onToggle={(scheme, enabled) =>
            onToggleTouchKeyboardScheme(scheme as TouchKeyboardScheme, enabled)
          }
        />
      )}
      <InputSchemeSelectorSection
        grouped={grouped}
        hidden={hasTouchKeyboardSchemes || !chineseSchemes}
        value={
          preferences.scheme === "shuangpin" || preferences.scheme === "wubi"
            ? preferences.scheme
            : "quanpin"
        }
        onChange={(scheme: InputSchemeSelectorValue) =>
          onPreferencesChange({ scheme, last_chinese_scheme: scheme })
        }
      />
      <InputSchemeDetailsSection
        grouped={grouped}
        scheme={preferences.scheme}
        shuangpinProfile={preferences.shuangpin_profile}
        macos={macos}
        hasTouchKeyboardSchemes={hasTouchKeyboardSchemes}
        macosShuangpinKeymap={macosShuangpinKeymap}
        onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
          onPreferencesChange({ shuangpin_profile })
        }
        onMacosShuangpinKeymapChange={onMacosShuangpinKeymapChange}
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
    </>
  );

  return grouped ? <GroupList title="方案">{content}</GroupList> : content;
}
