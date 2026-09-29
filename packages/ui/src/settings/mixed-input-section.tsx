import { Row, Select, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";

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

/** Shared mixed Chinese and English candidate controls: the leading rows of the 多语言候选 group on the 表达 page. */
export function MixedInputSection({ preferences, onChange }: MixedInputSectionProps) {
  return (
    <>
      <div role="group" aria-label="中英混输" className={settings.rowStack}>
        <Row title="中英混输" description="中文输入时在候选项中补充英文单词">
          <Switch
            checked={preferences.english}
            onChange={(english) => onChange({ ...preferences, english })}
          />
        </Row>
        <Row title="触发字符数" description="预编辑字母达到该长度后才出现英文候选项">
          <Select
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
          </Select>
        </Row>
      </div>
      {supplementalOptions.map(([key, label, description]) => (
        <Row key={key} title={label} description={description}>
          <Switch
            checked={preferences[key]}
            onChange={(checked) => onChange({ ...preferences, [key]: checked })}
          />
        </Row>
      ))}
    </>
  );
}
