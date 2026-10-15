import type { ReactNode } from "react";
import type { Preferences } from "../index";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { ChineseEnglishSpacingSection } from "./chinese-english-spacing-section";
import { CloudCandidatesSection } from "./cloud-candidates-section";
import { DefaultImeModeSection } from "./default-ime-mode-section";
import { FrequencySection } from "./frequency-section";
import type { FrequencyPreferences } from "./frequency-preferences";
import { ImeModeScopeSection } from "./ime-mode-scope-section";
import { InputModeHudSection } from "./input-mode-hud-section";
import { LearningSection } from "./learning-section";
import { MixedInputSection } from "./mixed-input-section";
import type { MixedInputPreferences } from "./mixed-input-preferences";
import { SingleCharacterOnlySection } from "./single-character-only-section";
import { SwitchRow } from "./switch-row";
import { TraditionalChineseOutputSection } from "./traditional-chinese-output-section";
import { WordCharacterSection } from "./word-character-section";
import type {
  NavigationPreferences,
  WordCharacterPreferences,
} from "./candidate-paging-preferences";

export type InputSharedSettingsTab = "mode" | "selection" | "candidates" | "output" | "advanced";

export interface InputSharedSettingsSectionProps {
  /** 桌面页按功能包装分区，手机缺省保持全部直列。 */
  renderGroup?: (tab: InputSharedSettingsTab, children: ReactNode) => ReactNode;
  preferences: Preferences;
  wordCharacter: WordCharacterPreferences;
  navigation: NavigationPreferences;
  frequency: FrequencyPreferences;
  ios: boolean;
  showInputModeHUD: boolean;
  /** macOS 菜单栏的输入模式状态项开关，跟在中英文切换提示之后。 */
  showInputModeStatusItem?: boolean;
  showModeScope: boolean;
  /** 中英混输、符号混输、emoji 和颜文字混输，放在「候选与联想」组的云候选之后。 */
  mixedInput?: MixedInputPreferences;
  onMixedInputChange?: (mixedInput: MixedInputPreferences) => void;
  /** 本版本带临时日文，决定是否列出日文混输开关；见 `MixedInputSectionProps.temporaryJapanese`。缺省为 true。 */
  temporaryJapanese?: boolean;
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
  /** 鸿蒙手机的「输入」页只把主要分组留在视野内：这里的每个分组都放进末尾「更多」分组的同一个「更多选项」折叠中，「中文输出」单选不放，因为该页以「中文字符集」提供它。 */
  foldIntoMore?: boolean;
  /** 宿主能按 OpenCC 变体做繁体转换（`HostCapabilities.traditional_chinese_variants`），「中文输出」单选列出台湾正体和台湾用词。 */
  traditionalChineseVariants?: boolean;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
}

/** 输入页共用控件，按功能分区组合。 */
export function InputSharedSettingsSection({
  renderGroup = (_tab, children) => children,
  preferences,
  wordCharacter,
  navigation,
  frequency,
  ios,
  showInputModeHUD,
  showInputModeStatusItem = false,
  showModeScope,
  mixedInput,
  onMixedInputChange,
  temporaryJapanese = true,
  beforeLearning,
  afterLearning,
  modeExtra,
  paging,
  outputExtra,
  beforeFrequency,
  afterFrequency,
  foldIntoMore = false,
  traditionalChineseVariants = false,
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
      variant={preferences.traditional_chinese_variant}
      variants={traditionalChineseVariants}
      scheme={preferences.scheme}
      onChange={onPreferencesChange}
    />
  );

  // 输入页按「基础 → 进阶」排列：先是每个人都会碰到的中英文和选词翻页，再是候选来源（云候选、中英混输与符号、emoji、颜文字混输、只出单字、整句联想和学习）和输出形式，进阶的快捷模式、模糊音、辅助码由页面经 beforeFrequency 放在调频之前。
  const groups = (
    <>
      {renderGroup(
        "mode",
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
          {showInputModeStatusItem && (
            <SwitchRow
              title="菜单栏显示中/英状态"
              description="在菜单栏常驻显示当前模式：中文时是所用方案（中、双、五、日等），英文时是「英」。不需要把「英」加成系统输入源，关掉切换提示后也能随时看到。改动在回到输入框后生效。中英文按应用记忆时，切到没有输入框的应用后，这里显示的仍是上一个应用的状态。"
              checked={preferences.input_mode_status_item ?? false}
              onChange={(input_mode_status_item) => onPreferencesChange({ input_mode_status_item })}
            />
          )}
          {modeExtra}
        </GroupList>,
      )}
      {/* 没有翻页键时（HarmonyOS 手机的翻页键在「外接键盘快捷键」页），这个分组只管选择。 */}
      {renderGroup(
        "selection",
        <GroupList title={foldIntoMore && !paging ? "选词" : "选词与翻页"}>
          {wordCharacterRows}
          {paging}
        </GroupList>,
      )}
      {renderGroup(
        "candidates",
        <GroupList title="候选与联想">
          {cloudCandidatesRow}
          {mixedInput && onMixedInputChange && (
            <MixedInputSection
              preferences={mixedInput}
              temporaryJapanese={temporaryJapanese}
              onChange={onMixedInputChange}
            />
          )}
          <SingleCharacterOnlySection
            value={preferences.single_character_only}
            onChange={(single_character_only) => onPreferencesChange({ single_character_only })}
          />
          {beforeLearning}
          {learningRow}
          {afterLearning}
        </GroupList>,
      )}
      {renderGroup(
        "output",
        <GroupList title="输出">
          {outputExtra}
          {!foldIntoMore && traditionalOutputRow}
          <ChineseEnglishSpacingSection
            value={preferences.chinese_english_spacing}
            onChange={(chinese_english_spacing) => onPreferencesChange({ chinese_english_spacing })}
          />
        </GroupList>,
      )}
      {renderGroup(
        "advanced",
        <>
          {beforeFrequency}
          <FrequencySection
            preferences={frequency}
            onChange={(nextFrequency) => onPreferencesChange({ frequency: nextFrequency })}
          />
          {afterFrequency}
        </>,
      )}
    </>
  );
  if (foldIntoMore)
    return (
      <GroupList title="更多">
        <MoreOptions>{groups}</MoreOptions>
      </GroupList>
    );
  return groups;
}
