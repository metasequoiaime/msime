import type { PreeditStyle, ShuangpinPreeditStyle } from "./preedit-preferences";

export type PreeditStyleSelectMode = "shuangpin" | "inline" | "candidate";
/** 选择器里的取值：行内、候选窗口预编辑的样式，或双拼预编辑的三档之一。 */
export type PreeditStyleOption = PreeditStyle | ShuangpinPreeditStyle;

export const preeditStyleOptions: Record<
  PreeditStyleSelectMode,
  readonly { value: PreeditStyleOption; label: string }[]
> = {
  shuangpin: [
    { value: "raw", label: "原始按键" },
    { value: "separated", label: "双拼分隔" },
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
