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

/** Shared mixed Chinese and English candidate controls. */
export function MixedInputSection({ preferences, onChange }: MixedInputSectionProps) {
  return (
    <>
      <div className="section" role="group" aria-label="中英混输">
        <label className="section-header">
          <span className="section-title">
            中英混输<small>中文输入时在候选项中补充英文单词</small>
          </span>
          <input
            className="toggle"
            type="checkbox"
            checked={preferences.english}
            onChange={(event) => onChange({ ...preferences, english: event.target.checked })}
          />
        </label>
        <div className="input-option-divider" />
        <label className="section-header frequency-option-row">
          <span className="section-title">
            触发字符数<small>预编辑字母达到该长度后才出现英文候选项</small>
          </span>
          <select
            aria-label="触发字符数"
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
          </select>
        </label>
      </div>
      {supplementalOptions.map(([key, label, description]) => (
        <div className="section" key={key}>
          <label className="section-header">
            <span className="section-title">
              {label}
              <small>{description}</small>
            </span>
            <input
              className="toggle"
              type="checkbox"
              checked={preferences[key]}
              onChange={(event) => onChange({ ...preferences, [key]: event.target.checked })}
            />
          </label>
        </div>
      ))}
    </>
  );
}
