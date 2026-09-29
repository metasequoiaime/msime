export type ChineseInputScheme = "quanpin" | "shuangpin" | "wubi";

export const chineseInputSchemeOptions = [
  { value: "quanpin", label: "全拼" },
  { value: "shuangpin", label: "双拼" },
  { value: "wubi", label: "五笔" },
] as const satisfies readonly { value: ChineseInputScheme; label: string }[];

export const japaneseInputSchemeOptions = [{ value: "romaji", label: "罗马音" }] as const;
