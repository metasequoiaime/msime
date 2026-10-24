import { SwitchRow } from "./switch-row";
import { SettingsRowStack } from "./settings-row-stack";
import { SelectRow } from "./select-row";

import { mixedInputControlState, type MixedInputPreferences } from "./mixed-input-preferences";

export {
  defaultMixedInput,
  mixedInputDefaultsFor,
  type MixedInputPreferences,
} from "./mixed-input-preferences";

const emojiDescription =
  "中文输入时在候选项中加入匹配的 emoji，紧跟在它描绘的那个词后面（如「美国」后面是 🇺🇸），对不上候选词的排在末尾";
const kaomojiDescription =
  "中文输入时在候选项中加入匹配的颜文字，紧跟在它描绘的那个词后面、排在同一个词的 emoji 之后，对不上候选词的排在末尾";

export interface MixedInputSectionProps {
  preferences: MixedInputPreferences;
  /** 宿主读触发字符数（`HostCapabilities.mixed_input_minimum_prefix`）时显示「触发字符数」；为假时（macOS、Windows、Linux 三个桌面宿主）它固定为 5，这一行不出现。缺省为真。 */
  showMinimumPrefix?: boolean;
  onChange: (preferences: MixedInputPreferences) => void;
}

/** 共享的中英混输候选控件：输入页「候选与联想」组里云候选之后的几行，即中英混输和 emoji 混输。符号、颜文字和日文混输在 `MixedInputMoreRows`，由页面收进「更多选项」。 */
export function MixedInputSection({
  preferences,
  showMinimumPrefix = true,
  onChange,
}: MixedInputSectionProps) {
  const state = mixedInputControlState(preferences);
  return (
    <>
      <SettingsRowStack role="group" aria-label="中英混输">
        <SwitchRow
          title="候选混输"
          description="中文输入时把英文、emoji、颜文字、符号和快捷短语混进候选；关掉后各开关原值保留，重开即恢复"
          checked={state.enabled}
          onChange={(enabled) => onChange({ ...preferences, enabled })}
        />
        <SwitchRow
          title="中英混输"
          description="中文输入时在候选项中补充英文单词"
          checked={preferences.english}
          disabled={state.englishDisabled}
          onChange={(english) => onChange({ ...preferences, english })}
        />
        <SelectRow
          title="首页英文候选数"
          description="中文输入时首页最多展示几个英文候选，放不下的接在候选列表末尾"
          disabled={state.firstPageCountDisabled}
          value={state.firstPageCount}
          onChange={(event) =>
            onChange({ ...preferences, first_page_count: Number(event.target.value) })
          }
        >
          {[1, 2, 3].map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectRow>
        {showMinimumPrefix && (
          <SelectRow
            title="触发字符数"
            description="达到该长度后补全英文；26 键中更短的完整词仍可选"
            disabled={state.minimumPrefixDisabled}
            value={preferences.minimum_prefix}
            onChange={(event) =>
              onChange({ ...preferences, minimum_prefix: Number(event.target.value) })
            }
          >
            {[1, 2, 3, 4, 5, 6, 7, 8].map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </SelectRow>
        )}
      </SettingsRowStack>
      <SwitchRow
        title="emoji 混输"
        description={emojiDescription}
        checked={preferences.emoji}
        disabled={state.englishDisabled}
        onChange={(emoji) => onChange({ ...preferences, emoji })}
      />
    </>
  );
}

export interface MixedInputMoreRowsProps {
  preferences: MixedInputPreferences;
  /** 本版本带临时日文（`HostCapabilities.edition.temporary_japanese`）；为 false 时不列出日文混输开关，因为 host-api 在这样的版本里始终把它关掉，和临时日语开关一样。缺省为 true。 */
  temporaryJapanese?: boolean;
  /** 宿主能把快捷短语混进普通候选（`HostCapabilities.quick_phrase_candidates`，macOS 和 Linux）；为 false 时不列出这两个开关。 */
  quickPhraseCandidates?: boolean;
  onChange: (preferences: MixedInputPreferences) => void;
}

/** 少有人改的混输开关：符号混输（默认开，要打完整名称才出现）、颜文字混输（默认关，M 模式另有入口）、快捷短语混输与调频（只有 macOS、Linux 提供入口）和实验性的日文混输。输入页把它们收进「候选与联想」组的「更多选项」。 */
export function MixedInputMoreRows({
  preferences,
  temporaryJapanese = true,
  quickPhraseCandidates = false,
  onChange,
}: MixedInputMoreRowsProps) {
  const state = mixedInputControlState(preferences);
  return (
    <>
      {quickPhraseCandidates && (
        <>
          <SwitchRow
            title="快捷短语混输"
            description="中文输入的编码恰好是某条快捷短语的编码时，短语排在普通候选之后；Shift+K 的 K 模式不受影响"
            checked={state.quickPhraseChecked}
            disabled={!state.enabled}
            onChange={(quick_phrase) => onChange({ ...preferences, quick_phrase })}
          />
          <SwitchRow
            title="快捷短语参与调频"
            description="常越过整组快捷短语选别的候选时整组后退，最多退到首页末位；选中组内短语时整组前移"
            checked={state.quickPhraseFrequencyChecked}
            disabled={!state.enabled || !state.quickPhraseChecked}
            onChange={(quick_phrase_frequency) =>
              onChange({ ...preferences, quick_phrase_frequency })
            }
          />
        </>
      )}
      <SwitchRow
        title="符号混输"
        description="打出符号的名称时，在候选项中加入对应的符号，紧跟在那个词后面（如「正负」后面是 ±、「平方米」后面是 ㎡）；要打完整的名称才出现"
        checked={state.symbolsChecked}
        disabled={state.englishDisabled}
        onChange={(symbols) => onChange({ ...preferences, symbols })}
      />
      <SwitchRow
        title="颜文字混输"
        description={kaomojiDescription}
        checked={preferences.kaomoji}
        disabled={state.englishDisabled}
        onChange={(kaomoji) => onChange({ ...preferences, kaomoji })}
      />
      {temporaryJapanese && (
        <SwitchRow
          title="日文混输（实验）"
          description="全拼、双拼里打出完整的日文罗马字时补充一个日文候选（如 ohayou → おはよう）。用的是临时日文的那份日文词典，词典不在时不生效"
          checked={state.japaneseChecked}
          disabled={state.englishDisabled}
          onChange={(japanese) => onChange({ ...preferences, japanese })}
        />
      )}
    </>
  );
}
