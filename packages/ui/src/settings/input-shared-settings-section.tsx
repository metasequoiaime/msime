import type { ReactNode } from "react";
import type { Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
import { CloudCandidatesSection } from "./cloud-candidates-section";
import { DefaultImeModeSection } from "./default-ime-mode-section";
import { FrequencySection, type FrequencyPreferences } from "./frequency-section";
import { ImeModeScopeSection } from "./ime-mode-scope-section";
import { InputModeHudSection } from "./input-mode-hud-section";
import { LearningSection } from "./learning-section";
import { MixedInputSection, type MixedInputPreferences } from "./mixed-input-section";
import { SingleCharacterOnlySection } from "./single-character-only-section";
import { TraditionalChineseOutputSection } from "./traditional-chinese-output-section";
import {
  WordCharacterSection,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./word-character-section";

export interface InputSharedSettingsSectionProps {
  preferences: Preferences;
  wordCharacter: WordCharacterPreferences;
  navigation: NavigationPreferences;
  frequency: FrequencyPreferences;
  ios: boolean;
  showInputModeHUD: boolean;
  showModeScope: boolean;
  /** 中英混输、emoji 和颜文字混输，放在「候选与联想」组的云候选之后。 */
  mixedInput?: MixedInputPreferences;
  onMixedInputChange?: (mixedInput: MixedInputPreferences) => void;
  /** 放在「候选与联想」组的「学习选词习惯」之前。 */
  beforeLearning?: ReactNode;
  /** 「候选与联想」组末尾的行。 */
  afterLearning?: ReactNode;
  /** 「中英文」组末尾的行。 */
  modeExtra?: ReactNode;
  /** 「选词与翻页」组里以词定字之后的翻页设置，两者互斥，所以放在同一组里同屏可见。 */
  paging?: ReactNode;
  outputExtra?: ReactNode;
  beforeFrequency?: ReactNode;
  afterFrequency?: ReactNode;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
}

/** Shared input controls of the settings page's 输入 page. */
export function InputSharedSettingsSection({
  preferences,
  wordCharacter,
  navigation,
  frequency,
  ios,
  showInputModeHUD,
  showModeScope,
  mixedInput,
  onMixedInputChange,
  beforeLearning,
  afterLearning,
  modeExtra,
  paging,
  outputExtra,
  beforeFrequency,
  afterFrequency,
  onPreferencesChange,
}: InputSharedSettingsSectionProps) {
  const wordCharacterRows = (
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
  );
  const learningRow = (
    <LearningSection
      value={preferences.learning}
      onChange={(learning) => onPreferencesChange({ learning })}
    />
  );
  const cloudCandidatesRow = (
    <CloudCandidatesSection
      value={preferences.cloud_candidates}
      onChange={(cloud_candidates) => onPreferencesChange({ cloud_candidates })}
    />
  );
  const traditionalOutputRow = (
    <TraditionalChineseOutputSection
      value={preferences.traditional_chinese_output}
      scheme={preferences.scheme}
      onChange={(traditional_chinese_output) => onPreferencesChange({ traditional_chinese_output })}
    />
  );

  // 输入页按「基础 → 进阶」排列：先是每个人都会碰到的中英文和选词翻页，再是候选来源（云候选、中英混输与 emoji、颜文字混输、只出单字、整句联想和学习）和输出形式，进阶的快捷模式、模糊音、辅助码由页面经 beforeFrequency 放在调频之前。
  return (
    <>
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
        {modeExtra}
      </GroupList>
      <GroupList title="选词与翻页">
        {wordCharacterRows}
        {paging}
      </GroupList>
      <GroupList title="候选与联想">
        {cloudCandidatesRow}
        {mixedInput && onMixedInputChange && (
          <MixedInputSection preferences={mixedInput} onChange={onMixedInputChange} />
        )}
        <SingleCharacterOnlySection
          value={preferences.single_character_only}
          onChange={(single_character_only) => onPreferencesChange({ single_character_only })}
        />
        {beforeLearning}
        {learningRow}
        {afterLearning}
      </GroupList>
      <GroupList title="输出">
        {outputExtra}
        {traditionalOutputRow}
      </GroupList>
      {beforeFrequency}
      <FrequencySection
        preferences={frequency}
        onChange={(nextFrequency) => onPreferencesChange({ frequency: nextFrequency })}
      />
      {afterFrequency}
    </>
  );
}
