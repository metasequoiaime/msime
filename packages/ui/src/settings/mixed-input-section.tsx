import { SwitchRow } from "./switch-row";
import * as settings from "./settings-style";
import { SelectRow } from "./select-row";

export type MixedInputPreferences = {
  english: boolean;
  minimum_prefix: number;
  emoji: boolean;
  kaomoji: boolean;
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
    "中文输入时在候选项中加入匹配的 emoji（位于英文候选之后；云候选与 AI 联想会使其相应顺移）",
  ],
  [
    "kaomoji",
    "颜文字混输",
    "中文输入时在候选项中加入匹配的颜文字（排在 emoji 之后；云候选与 AI 联想会使其相应顺移）",
  ],
];

export interface MixedInputSectionProps {
  preferences: MixedInputPreferences;
  onChange: (preferences: MixedInputPreferences) => void;
}

/** 共享的中英混输候选控件：「标点与翻译」页「多语言与释义」组开头的几行。 */
export function MixedInputSection({ preferences, onChange }: MixedInputSectionProps) {
  return (
    <>
      <div role="group" aria-label="中英混输" className={settings.rowStack}>
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
      </div>
      {supplementalOptions.map(([key, label, description]) => (
        <SwitchRow
          key={key}
          title={label}
          description={description}
          checked={preferences[key]}
          onChange={(checked) => onChange({ ...preferences, [key]: checked })}
        />
      ))}
    </>
  );
}
