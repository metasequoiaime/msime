import type { ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { ChineseEnglishSpacingSection } from "./chinese-english-spacing-section";
import { CloudCandidatesSection } from "./cloud-candidates-section";
import { DefaultImeModeSection } from "./default-ime-mode-section";
import { FrequencySection } from "./frequency-section";
import type { FrequencyPreferences } from "./frequency-preferences";
import { ImeModeScopeSection } from "./ime-mode-scope-section";
import { InputModeHudSection } from "./input-mode-hud-section";
import { LearningSection } from "./learning-section";
import { MultilingualInputSection } from "./multilingual-input-section";
import { MixedInputMoreRows, MixedInputSection } from "./mixed-input-section";
import type { MixedInputPreferences } from "./mixed-input-preferences";
import { SingleCharacterOnlySection } from "./single-character-only-section";
import { CaretEditSuffixSection } from "./caret-edit-suffix-section";
import { NextWordPredictionSection } from "./next-word-prediction-section";
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
  /** Linux 可选的聚焦提示，与真正切换中英文时的提示分开。 */
  showInputModeFocusHUD?: boolean;
  showInputModeCornerHUD?: boolean;
  /** macOS 菜单栏的输入模式状态项开关，跟在中英文切换提示之后。 */
  showInputModeStatusItem?: boolean;
  showModeScope: boolean;
  /** 中英混输和 emoji 混输放在「候选与联想」组的云候选之后，符号、颜文字和日文混输收进组末的「更多选项」。 */
  mixedInput?: MixedInputPreferences;
  onMixedInputChange?: (mixedInput: MixedInputPreferences) => void;
  /** 宿主读中英混输的触发字符数（`HostCapabilities.mixed_input_minimum_prefix`），给出「触发字符数」一行。缺省为 true。 */
  showMixedInputMinimumPrefix?: boolean;
  /** 本版本带临时日文，决定是否列出日文混输开关；见 `MixedInputMoreRowsProps.temporaryJapanese`。缺省为 true。 */
  temporaryJapanese?: boolean;
  /** 本版本提供全拼时才显示连续多语言设置。 */
  showMultilingualInput?: boolean;
  /** 宿主能把快捷短语混进普通候选（`HostCapabilities.quick_phrase_candidates`），决定是否列出快捷短语混输与调频开关。缺省为 false。 */
  quickPhraseCandidates?: boolean;
  /** 宿主提供「移动光标后编辑光标后的部分」（`HostCapabilities.caret_edit_suffix`，桌面宿主），决定是否在「更多选项」开头、快捷短语混输之前列出它。缺省为 false。 */
  caretEditSuffix?: boolean;
  /** 宿主显示上屏后联想（`HostCapabilities.next_word_prediction`），决定是否在学习开关之后列出「上屏后联想」。桌面宿主没有联想栏按键，只列开关。缺省为 false。 */
  nextWordPrediction?: boolean;
  /** 放在「候选与联想」组的「学习选词习惯」之前。 */
  beforeLearning?: ReactNode;
  /** 「候选与联想」组里学习开关之后的行。 */
  afterLearning?: ReactNode;
  /** 页面另给的少有人改的候选行（本地整句联想），和符号、颜文字、日文混输、只出单字一起收进「候选与联想」组末的「更多选项」。 */
  candidatesMore?: ReactNode;
  /** 「中英文」组末尾的行。 */
  modeExtra?: ReactNode;
  /** 「选词与翻页」组里以词定字之后的翻页设置，两者互斥，所以放在同一组里同屏可见。 */
  paging?: ReactNode;
  /** 「输出」组直接显示的附加行（全角输入的起始状态）。 */
  outputExtra?: ReactNode;
  /** 已接入跨次空格的宿主提供独立开关和应用排除名单。 */
  crossCommitSpacing?: ReactNode;
  beforeFrequency?: ReactNode;
  afterFrequency?: ReactNode;
  /** 把这里的共享分组整体收进末尾「更多」分组的同一个「更多选项」折叠中；折叠时不显示「中文输出」单选，由调用方提供等价的中文字符集控件。 */
  foldIntoMore?: boolean;
  /** 是否显示「中文输出」单选；HarmonyOS 手机已经在「中文」组提供「中文字符集」。 */
  showTraditionalOutput?: boolean;
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
  showInputModeFocusHUD = false,
  showInputModeCornerHUD = false,
  showInputModeStatusItem = false,
  showModeScope,
  mixedInput,
  onMixedInputChange,
  showMixedInputMinimumPrefix = true,
  temporaryJapanese = true,
  showMultilingualInput = false,
  quickPhraseCandidates = false,
  caretEditSuffix = false,
  nextWordPrediction = false,
  beforeLearning,
  afterLearning,
  candidatesMore,
  modeExtra,
  paging,
  outputExtra,
  crossCommitSpacing,
  beforeFrequency,
  afterFrequency,
  foldIntoMore = false,
  showTraditionalOutput = true,
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
      provider={preferences.cloud_candidate_provider}
      onProviderChange={(cloud_candidate_provider) =>
        onPreferencesChange({ cloud_candidate_provider })
      }
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

  // 少有人改的候选行收进组末的「更多选项」；整体折叠时这里不再套第二层，混输各行和只出单字仍在学习开关之前。
  const candidatesMoreRows = (
    <>
      {caretEditSuffix && (
        <CaretEditSuffixSection
          value={preferences.caret_edit_suffix}
          onChange={(caret_edit_suffix) => onPreferencesChange({ caret_edit_suffix })}
        />
      )}
      {mixedInput && onMixedInputChange && (
        <MixedInputMoreRows
          preferences={mixedInput}
          temporaryJapanese={temporaryJapanese}
          quickPhraseCandidates={quickPhraseCandidates}
          onChange={onMixedInputChange}
        />
      )}
      {showMultilingualInput && (
        <MultilingualInputSection
          preferences={preferences.multilingual_input}
          temporaryJapanese={temporaryJapanese}
          onChange={(multilingual_input) => onPreferencesChange({ multilingual_input })}
        />
      )}
      <SingleCharacterOnlySection
        value={preferences.single_character_only}
        onChange={(single_character_only) => onPreferencesChange({ single_character_only })}
      />
      {candidatesMore}
    </>
  );

  // 输入页按「基础 → 进阶」排列：先是每个人都会碰到的中英文和选词翻页，再是候选来源（云候选、中英混输、emoji、神经联想和学习，其余收进「更多选项」）和输出形式，进阶的快捷模式、模糊音、辅助码由页面经 beforeFrequency 放在调频之前。
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
              corner={
                showInputModeCornerHUD
                  ? {
                      value: preferences.input_mode_corner_hud,
                      onChange: (input_mode_corner_hud) =>
                        onPreferencesChange({ input_mode_corner_hud }),
                    }
                  : undefined
              }
              focus={
                showInputModeFocusHUD
                  ? {
                      value: preferences.input_mode_focus_hud,
                      onChange: (input_mode_focus_hud) =>
                        onPreferencesChange({ input_mode_focus_hud }),
                    }
                  : undefined
              }
            />
          )}
          {showInputModeStatusItem && (
            <SwitchRow
              title="菜单栏显示中/英状态"
              description="在菜单栏常驻显示当前模式：中文时是所用方案（中、双、五等），英文时是「英」。"
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
          <SwitchRow
            title="二三候选"
            description="开启后，组字时按 ; 选第二个候选、按 ' 选第三个候选；组字中的 ' 不再作为音节分隔符。"
            checked={preferences.second_third_candidate?.enabled ?? false}
            onChange={(enabled) =>
              onPreferencesChange({
                second_third_candidate: { enabled, keys: "semicolon_quote" },
              })
            }
          />
          <SwitchRow
            title="退格键回退已选候选词"
            description="关闭后，退格只删除剩余拼写，保留已选文字；拼写删空时上屏已选文字。"
            checked={preferences.backspace_reverts_selection ?? true}
            onChange={(backspace_reverts_selection) =>
              onPreferencesChange({ backspace_reverts_selection })
            }
          />
          <SwitchRow
            title="Esc 保留已选词"
            description="造词时按 Esc 只清除还没选的拼音、保留已选的词，再按一次 Esc 整体取消（小狼毫风格）。"
            checked={preferences.escape_keeps_selected_word ?? false}
            onChange={(escape_keeps_selected_word) =>
              onPreferencesChange({ escape_keeps_selected_word })
            }
          />
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
              showMinimumPrefix={showMixedInputMinimumPrefix}
              onChange={onMixedInputChange}
            />
          )}
          {foldIntoMore && candidatesMoreRows}
          {beforeLearning}
          {learningRow}
          <SwitchRow
            title="回车学英文词"
            description="按回车上屏输入的英文时，把这个单词写入英文词库，之后可作为候选出现。"
            checked={preferences.enter_learns_english_word ?? true}
            onChange={(enter_learns_english_word) =>
              onPreferencesChange({ enter_learns_english_word })
            }
          />
          {nextWordPrediction && (
            <NextWordPredictionSection
              value={preferences.next_word_prediction}
              buttons={preferences.next_word_prediction_buttons}
              showButtons={false}
              onChange={onPreferencesChange}
            />
          )}
          {afterLearning}
          {!foldIntoMore && <MoreOptions>{candidatesMoreRows}</MoreOptions>}
        </GroupList>,
      )}
      {renderGroup(
        "output",
        <GroupList title="输出">
          {foldIntoMore && outputExtra}
          {!foldIntoMore && showTraditionalOutput && traditionalOutputRow}
          <ChineseEnglishSpacingSection
            value={preferences.chinese_english_spacing}
            onChange={(chinese_english_spacing) => onPreferencesChange({ chinese_english_spacing })}
          />
          {crossCommitSpacing}
          {!foldIntoMore && outputExtra}
        </GroupList>,
      )}
      {renderGroup(
        "advanced",
        <>
          {beforeFrequency}
          <FrequencySection
            preferences={frequency}
            learning={preferences.learning}
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
