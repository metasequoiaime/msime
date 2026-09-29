import { GroupList, Row, Select } from "../core/platform-controls";

export type FrequencyPreferences = {
  mode: "disabled" | "pin" | "halve" | "linear" | "promote";
  trigger_count: number;
  linear_step: number;
};

export const defaultFrequency: FrequencyPreferences = {
  mode: "promote",
  trigger_count: 1,
  linear_step: 1,
};

export interface FrequencySectionProps {
  preferences: FrequencyPreferences;
  onChange: (preferences: FrequencyPreferences) => void;
}

/** Shared pinyin frequency adjustment controls: the 拼音方案调频 group. */
export function FrequencySection({ preferences, onChange }: FrequencySectionProps) {
  return (
    <GroupList title="拼音方案调频">
      <Row title="调频方式">
        <Select
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
        </Select>
      </Row>
      {(
        [
          ["trigger_count", "触发频次(第几次上屏触发)"],
          ["linear_step", "线性调频步长"],
        ] as const
      ).map(([key, label]) => (
        <Row key={key} title={label}>
          <Select
            value={preferences[key]}
            onChange={(event) => onChange({ ...preferences, [key]: Number(event.target.value) })}
          >
            {/* A persisted value above the standard range stays selectable rather than silently snapping to 6. */}
            {[1, 2, 3, 4, 5, 6, ...(preferences[key] > 6 ? [preferences[key]] : [])].map(
              (value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ),
            )}
          </Select>
        </Row>
      ))}
    </GroupList>
  );
}
