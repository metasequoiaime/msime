import { Select } from "../core/platform-controls";

export type PreeditStyle = "raw" | "pinyin" | "empty";
export type PreeditStyleSelectMode = "shuangpin" | "inline" | "candidate";

const preeditStyleOptions: Record<
  PreeditStyleSelectMode,
  readonly { value: PreeditStyle; label: string }[]
> = {
  shuangpin: [
    { value: "raw", label: "原始按键" },
    { value: "pinyin", label: "拼音分词" },
  ],
  inline: [
    { value: "raw", label: "原始按键" },
    { value: "pinyin", label: "拼音分词" },
    { value: "empty", label: "不显示" },
  ],
  candidate: [
    { value: "pinyin", label: "拼音分词" },
    { value: "empty", label: "不显示" },
  ],
};

export interface PreeditStyleSelectProps {
  mode: PreeditStyleSelectMode;
  value: PreeditStyle;
  onChange: (value: PreeditStyle) => void;
}

/** Shared presentation choices for inline and candidate-window preedit controls. */
export function PreeditStyleSelect({ mode, value, onChange }: PreeditStyleSelectProps) {
  return (
    <Select value={value} onChange={(event) => onChange(event.target.value as PreeditStyle)}>
      {preeditStyleOptions[mode].map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </Select>
  );
}
