import { GroupList } from "../core/platform-controls";
import { SelectRow } from "./select-row";

import { frequencyValueOptions, type FrequencyPreferences } from "./frequency-preferences";

export { defaultFrequency, type FrequencyPreferences } from "./frequency-preferences";

export interface FrequencySectionProps {
  preferences: FrequencyPreferences;
  /** 「学习选词习惯」是否打开。关着时 Engine 不按选词调整顺序，调频的各行都不起作用，整组不画；值留在文档里，重新打开学习后原样出现。缺省为 true。 */
  learning?: boolean;
  onChange: (preferences: FrequencyPreferences) => void;
}

/** 共享拼音调频控件，组成「拼音方案调频」组。三行按条件出现：学习打开才有调频方式，方式不是「关闭」才有触发频次，方式是「线性调频」才有步长（`ranking.rs` 只在线性调频时读它）。这三项会上云，所以不砍成固定值，只在不起作用时藏起来。 */
export function FrequencySection({
  preferences,
  learning = true,
  onChange,
}: FrequencySectionProps) {
  if (!learning) return null;
  const valueRow = (key: "trigger_count" | "linear_step", label: string) => (
    <SelectRow
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
  );
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
      {preferences.mode !== "disabled" && valueRow("trigger_count", "触发频次(第几次上屏触发)")}
      {preferences.mode === "linear" && valueRow("linear_step", "线性调频步长")}
    </GroupList>
  );
}
