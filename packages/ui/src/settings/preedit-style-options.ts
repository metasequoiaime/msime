import type { PreeditStyle } from "./preedit-preferences";

export type PreeditStyleSelectMode = "shuangpin" | "inline" | "candidate";

export const preeditStyleOptions: Record<
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
