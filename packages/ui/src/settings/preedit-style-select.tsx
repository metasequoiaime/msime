import {
  preeditStyleOptions,
  type PreeditStyleOption,
  type PreeditStyleSelectMode,
} from "./preedit-style-options";
import { Select } from "../core/platform-controls";

export type { PreeditStyle } from "./preedit-preferences";
export type { PreeditStyleOption, PreeditStyleSelectMode } from "./preedit-style-options";

export interface PreeditStyleSelectProps {
  mode: PreeditStyleSelectMode;
  value: PreeditStyleOption;
  onChange: (value: PreeditStyleOption) => void;
}

/** 行内、双拼与候选窗口预编辑共用的样式选择器。 */
export function PreeditStyleSelect({ mode, value, onChange }: PreeditStyleSelectProps) {
  return (
    <Select value={value} onChange={(event) => onChange(event.target.value as PreeditStyleOption)}>
      {preeditStyleOptions[mode].map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </Select>
  );
}
