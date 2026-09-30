import type { ReactNode } from "react";
import type { Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
import { CloudCandidatesSection } from "./cloud-candidates-section";
import { CandidateEnglishGlossSection } from "./candidate-english-gloss-section";
import { DefaultImeModeSection } from "./default-ime-mode-section";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { FrequencySection, type FrequencyPreferences } from "./frequency-section";
import { ImeModeScopeSection } from "./ime-mode-scope-section";
import { InputModeHudSection } from "./input-mode-hud-section";
import { LearningSection } from "./learning-section";
import { TraditionalChineseOutputSection } from "./traditional-chinese-output-section";
import {
  WordCharacterSection,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./word-character-section";

export interface InputSharedSettingsSectionProps {
  grouped?: boolean;
  preferences: Preferences;
  wordCharacter: WordCharacterPreferences;
  navigation: NavigationPreferences;
  frequency: FrequencyPreferences;
  ios: boolean;
  showInputModeHUD: boolean;
  showModeScope: boolean;
  showCandidateEnglishGloss?: boolean;
  showEnglishSuggestions?: boolean;
  beforeLearning?: ReactNode;
  beforeLanguage?: ReactNode;
  outputExtra?: ReactNode;
  beforeFrequency?: ReactNode;
  afterFrequency?: ReactNode;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
}

/** Shared input controls used by the settings page and the embedded input panel. */
export function InputSharedSettingsSection({
  grouped = false,
  preferences,
  wordCharacter,
  navigation,
  frequency,
  ios,
  showInputModeHUD,
  showModeScope,
  showCandidateEnglishGloss = false,
  showEnglishSuggestions = false,
  beforeLearning,
  beforeLanguage,
  outputExtra,
  beforeFrequency,
  afterFrequency,
  onPreferencesChange,
}: InputSharedSettingsSectionProps) {
  const wordAndLearning = (
    <>
      <WordCharacterSection
        preferences={wordCharacter}
        navigation={navigation}
        ios={ios}
        onChange={({ wordCharacter: nextWordCharacter, navigation: nextNavigation }) =>
          onPreferencesChange({
            word_character: nextWordCharacter,
            navigation: nextNavigation,
          })
        }
      />
      {beforeLearning}
      <LearningSection
        value={preferences.learning}
        onChange={(learning) => onPreferencesChange({ learning })}
      />
    </>
  );

  const modeAndLanguage = grouped ? (
    <GroupList title="中英文">
      <DefaultImeModeSection
        value={preferences.default_ime_mode}
        onChange={(default_ime_mode) => onPreferencesChange({ default_ime_mode })}
      />
      {showModeScope && (
        <ImeModeScopeSection
          value={preferences.ime_mode_scope}
          onChange={(ime_mode_scope) => onPreferencesChange({ ime_mode_scope })}
        />
      )}
      {showInputModeHUD && (
        <InputModeHudSection
          value={preferences.input_mode_hud}
          onChange={(input_mode_hud) => onPreferencesChange({ input_mode_hud })}
        />
      )}
    </GroupList>
  ) : (
    <>
      {beforeLanguage}
      {showInputModeHUD && (
        <InputModeHudSection
          value={preferences.input_mode_hud}
          onChange={(input_mode_hud) => onPreferencesChange({ input_mode_hud })}
        />
      )}
      {showCandidateEnglishGloss && (
        <CandidateEnglishGlossSection
          value={preferences.candidate_english_gloss}
          onChange={(candidate_english_gloss) => onPreferencesChange({ candidate_english_gloss })}
        />
      )}
      {showEnglishSuggestions && (
        <EnglishSuggestionsSection
          value={preferences.english_suggestions}
          onChange={(english_suggestions) => onPreferencesChange({ english_suggestions })}
        />
      )}
      <DefaultImeModeSection
        value={preferences.default_ime_mode}
        onChange={(default_ime_mode) => onPreferencesChange({ default_ime_mode })}
      />
      {showModeScope && (
        <ImeModeScopeSection
          value={preferences.ime_mode_scope}
          onChange={(ime_mode_scope) => onPreferencesChange({ ime_mode_scope })}
        />
      )}
    </>
  );

  const output = grouped ? (
    <GroupList title="输出">
      {outputExtra}
      <TraditionalChineseOutputSection
        value={preferences.traditional_chinese_output}
        onChange={(traditional_chinese_output) =>
          onPreferencesChange({ traditional_chinese_output })
        }
      />
      <CloudCandidatesSection
        value={preferences.cloud_candidates}
        onChange={(cloud_candidates) => onPreferencesChange({ cloud_candidates })}
      />
    </GroupList>
  ) : (
    <>
      <TraditionalChineseOutputSection
        value={preferences.traditional_chinese_output}
        onChange={(traditional_chinese_output) =>
          onPreferencesChange({ traditional_chinese_output })
        }
      />
      <CloudCandidatesSection
        value={preferences.cloud_candidates}
        onChange={(cloud_candidates) => onPreferencesChange({ cloud_candidates })}
      />
    </>
  );

  return (
    <>
      {grouped ? <GroupList title="选词">{wordAndLearning}</GroupList> : wordAndLearning}
      {modeAndLanguage}
      {output}
      {beforeFrequency}
      <FrequencySection
        preferences={frequency}
        onChange={(nextFrequency) => onPreferencesChange({ frequency: nextFrequency })}
      />
      {afterFrequency}
    </>
  );
}
