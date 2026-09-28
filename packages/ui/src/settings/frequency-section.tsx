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

/** Shared pinyin frequency adjustment controls. */
export function FrequencySection({ preferences, onChange }: FrequencySectionProps) {
  const options = (key: "trigger_count" | "linear_step") =>
    [1, 2, 3, 4, 5, 6, ...(preferences[key] > 6 ? [preferences[key]] : [])].map((value) => (
      <option key={value} value={value}>
        {value}
      </option>
    ));

  return (
    <div className="section" role="group" aria-labelledby="frequency-title">
      <div className="section-title" id="frequency-title">
        拼音方案调频
      </div>
      <div className="frequency-option-content">
        <label className="section-header frequency-option-row">
          <span className="section-title">调频方式</span>
          <select
            aria-label="调频方式"
            value={preferences.mode}
            onChange={(event) =>
              onChange({
                ...preferences,
                mode: event.target.value as FrequencyPreferences["mode"],
              })
            }
          >
            <option value="disabled">关闭</option>
            <option value="pin">一次置顶</option>
            <option value="halve">折半调频</option>
            <option value="linear">线性调频</option>
            <option value="promote">一次置前</option>
          </select>
        </label>
        <div className="input-option-divider" />
        <label className="section-header frequency-option-row">
          <span className="section-title">触发频次(第几次上屏触发)</span>
          <select
            aria-label="触发频次(第几次上屏触发)"
            value={preferences.trigger_count}
            onChange={(event) =>
              onChange({ ...preferences, trigger_count: Number(event.target.value) })
            }
          >
            {options("trigger_count")}
          </select>
        </label>
        <div className="input-option-divider" />
        <label className="section-header frequency-option-row">
          <span className="section-title">线性调频步长</span>
          <select
            aria-label="线性调频步长"
            value={preferences.linear_step}
            onChange={(event) =>
              onChange({ ...preferences, linear_step: Number(event.target.value) })
            }
          >
            {options("linear_step")}
          </select>
        </label>
      </div>
    </div>
  );
}
