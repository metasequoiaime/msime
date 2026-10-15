import { GroupList } from "../core/platform-controls";
import { SelectRow } from "./select-row";

import { frequencyValueOptions, type FrequencyPreferences } from "./frequency-preferences";

export { defaultFrequency, type FrequencyPreferences } from "./frequency-preferences";

export interface FrequencySectionProps {
  preferences: FrequencyPreferences;
  onChange: (preferences: FrequencyPreferences) => void;
}

/** 共享拼音调频控件，组成「拼音方案调频」组。 */
export function FrequencySection({ preferences, onChange }: FrequencySectionProps) {
  return (
    <GroupList title="拼音方案调频">
      <SelectRow
        title="调频方式"
        value={preferences.mode}
        onChange={(event) =>
          onChange({ ...preferences, mode: event.target.value as FrequencyPreferences["mode"] })
        }
      >
        <option value="disabled">关闭</option>
        <option value="pin">一次置顶</option>
        <option value="halve">折半调频</option>
        <option value="linear">线性调频</option>
        <option value="promote">一次置前</option>
      </SelectRow>
      {(
        [
          ["trigger_count", "触发频次(第几次上屏触发)"],
          ["linear_step", "线性调频步长"],
        ] as const
      ).map(([key, label]) => (
        <SelectRow
          key={key}
          title={label}
          value={preferences[key]}
          onChange={(event) => onChange({ ...preferences, [key]: Number(event.target.value) })}
        >
          {frequencyValueOptions(preferences[key]).map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectRow>
      ))}
    </GroupList>
  );
}
