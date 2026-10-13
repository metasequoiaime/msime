import { SwitchRow } from "./switch-row";
import { SettingsRowStack } from "./settings-row-stack";
import { SelectRow } from "./select-row";

export type MixedInputPreferences = {
  english: boolean;
  minimum_prefix: number;
  emoji: boolean;
  kaomoji: boolean;
  /** 日文混输（实验）。后加的可选成员：Rust 端关着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按关处理；也因此不放进 `defaultMixedInput`。 */
  japanese?: boolean;
  /** 符号混输（打拼音出符号）。后加的可选成员：Rust 端开着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按开处理；也因此不放进 `defaultMixedInput`。 */
  symbols?: boolean;
};

export const defaultMixedInput: MixedInputPreferences = {
  english: true,
  minimum_prefix: 5,
  emoji: false,
  kaomoji: false,
};

const supplementalOptions: readonly ["emoji" | "kaomoji", string, string][] = [
  [
    "emoji",
    "emoji 混输",
    "中文输入时在候选项中加入匹配的 emoji，紧跟在它描绘的那个词后面（如「美国」后面是 🇺🇸），对不上候选词的排在末尾",
  ],
  [
    "kaomoji",
    "颜文字混输",
    "中文输入时在候选项中加入匹配的颜文字，紧跟在它描绘的那个词后面、排在同一个词的 emoji 之后，对不上候选词的排在末尾",
  ],
];

export interface MixedInputSectionProps {
  preferences: MixedInputPreferences;
  /** 本版本带临时日文（`HostCapabilities.edition.temporary_japanese`）；为 false 时不列出日文混输开关，因为 host-api 在这样的版本里始终把它关掉，和临时日语开关一样。缺省为 true。 */
  temporaryJapanese?: boolean;
  onChange: (preferences: MixedInputPreferences) => void;
}

/** 共享的中英混输候选控件：输入页「候选与联想」组里云候选之后的几行。 */
export function MixedInputSection({
  preferences,
  temporaryJapanese = true,
  onChange,
}: MixedInputSectionProps) {
  return (
    <>
      <SettingsRowStack role="group" aria-label="中英混输">
        <SwitchRow
          title="中英混输"
          description="中文输入时在候选项中补充英文单词"
          checked={preferences.english}
          onChange={(english) => onChange({ ...preferences, english })}
        />
        <SelectRow
          title="触发字符数"
          description="预编辑字母达到该长度后才出现英文候选项"
          disabled={!preferences.english}
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
      </SettingsRowStack>
      <SwitchRow
        title="符号混输"
        description="打出符号的名称时，在候选项中加入对应的符号，紧跟在那个词后面（如「正负」后面是 ±、「平方米」后面是 ㎡）；要打完整的名称才出现"
        checked={preferences.symbols ?? true}
        onChange={(symbols) => onChange({ ...preferences, symbols })}
      />
      {supplementalOptions.map(([key, label, description]) => (
        <SwitchRow
          key={key}
          title={label}
          description={description}
          checked={preferences[key]}
          onChange={(checked) => onChange({ ...preferences, [key]: checked })}
        />
      ))}
      {temporaryJapanese && (
        <SwitchRow
          title="日文混输（实验）"
          description="全拼、双拼里打出完整的日文罗马字时补充一个日文候选（如 ohayou → おはよう）。用的是临时日文的那份日文词典，词典不在时不生效"
          checked={preferences.japanese ?? false}
          onChange={(japanese) => onChange({ ...preferences, japanese })}
        />
      )}
    </>
  );
}
