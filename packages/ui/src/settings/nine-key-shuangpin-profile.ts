import {
  shuangpinCustomProblemMessage,
  shuangpinCustomUnits,
  type ShuangpinCustomProblem,
  type ShuangpinCustomProfile,
} from "./shuangpin-custom-profile";

export type BuiltinNineKeyShuangpinProfile = "migala3" | "migala2" | "gushi2_low";
export type NineKeyShuangpinProfile = BuiltinNineKeyShuangpinProfile | "custom";

/** 与既有 C ABI 的请求、响应一致；表和错误均由引擎计算。 */
export interface NineKeyShuangpinLayout {
  effective: NineKeyShuangpinProfile;
  table: ShuangpinCustomProfile;
  problem: ShuangpinCustomProblem | null;
  /** 按电话键盘 1 到 9、0 的顺序给屏幕键盘显示的键面。 */
  keys?: readonly NineKeyShuangpinKeyFace[];
}

export interface NineKeyShuangpinKeyFace {
  key: string;
  initials: readonly string[];
  finals: readonly string[];
  zero_initial: boolean;
}

export type NineKeyShuangpinLayoutReader = (request: {
  profile: NineKeyShuangpinProfile;
  table?: ShuangpinCustomProfile;
}) => Promise<NineKeyShuangpinLayout>;

export const builtinNineKeyShuangpinProfiles: readonly [BuiltinNineKeyShuangpinProfile, string][] =
  [
    ["migala3", "米旮旯 3"],
    ["migala2", "米旮旯 2"],
    ["gushi2_low", "辜氏 2 低重码"],
  ];

/** 只用于列出编辑格子，映射值和合法性不在页面内重复实现。 */
export const nineKeyShuangpinUnits = {
  ...shuangpinCustomUnits,
  initials: [
    "b",
    "p",
    "m",
    "f",
    "d",
    "t",
    "n",
    "l",
    "g",
    "k",
    "h",
    "j",
    "q",
    "x",
    "r",
    "z",
    "c",
    "s",
    "y",
    "w",
    "zh",
    "ch",
    "sh",
  ],
};

export function nineKeyShuangpinProblemMessage(problem: ShuangpinCustomProblem): string {
  if (problem.kind !== "invalid_key") return shuangpinCustomProblemMessage(problem);
  const part = problem.part === "initials" ? "声母" : problem.part === "finals" ? "韵母" : "零声母";
  const rule = problem.part === "zero_initials" ? "两个 0–9 的数字" : "一个 0–9 的数字";
  return `${part}「${problem.unit ?? ""}」的键「${problem.key ?? ""}」不能用：要${rule}。`;
}
