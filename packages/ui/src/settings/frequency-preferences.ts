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

/** 常用数值按 1 到 6 排列，已保存的更大值仍需保持可选。 */
export function frequencyValueOptions(current: number): number[] {
  return [1, 2, 3, 4, 5, 6, ...(current > 6 ? [current] : [])];
}
