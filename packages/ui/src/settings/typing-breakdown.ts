import { withUnknown, type TypingBreakdown } from "./typing-speed";

type StatisticsBreakdownSource = {
  total: number;
  days: Record<string, number>;
  detail?: Partial<TypingBreakdown>;
  dailyDetails?: Record<string, Partial<TypingBreakdown>>;
};

export function scopedBreakdown(
  statistics: StatisticsBreakdownSource,
  keys: string[] | null,
): TypingBreakdown {
  if (keys === null) return withUnknown(statistics.detail, statistics.total);
  const result: TypingBreakdown = { characters: {}, sources: {} };
  for (const key of keys) {
    const detail = withUnknown(statistics.dailyDetails?.[key], statistics.days[key] ?? 0);
    for (const [id, value] of Object.entries(detail.characters))
      result.characters[id] = (result.characters[id] ?? 0) + value;
    for (const [id, value] of Object.entries(detail.sources))
      result.sources[id] = (result.sources[id] ?? 0) + value;
  }
  return result;
}

/** 整体中带标签的一部分，即统计概览图表绘制的单元。 */
export type BreakdownShare = { title: string; count: number };

function count(values: Record<string, number>, key: string): number {
  const value = values[key];
  return typeof value === "number" && Number.isFinite(value) ? Math.max(0, value) : 0;
}

/** 按给定顺序保留非空的分组。 */
function shares(groups: [string, number][]): BreakdownShare[] {
  return groups.filter(([, value]) => value > 0).map(([title, value]) => ({ title, count: value }));
}

/**
 * 统计概览上的「输入构成」：汉字、英文、符号（数字、标点和符号）、表情，以及「其他」（其他文字加上未分类的剩余部分）。只列出有字符的分组。分组方式与 Android 的 `TypingStatisticsSummary.composition` 一致。
 */
export function summaryComposition(characters: Record<string, number>): BreakdownShare[] {
  return shares([
    ["汉字", count(characters, "han")],
    ["英文", count(characters, "latin")],
    [
      "符号",
      count(characters, "number") + count(characters, "punctuation") + count(characters, "symbol"),
    ],
    ["表情", count(characters, "emoji")],
    ["其他", count(characters, "otherLetter") + count(characters, "unknown")],
  ]);
}

/** 不属于文字输入方式的来源：AI 改写、回复建议和未分类的剩余部分。 */
const notInputMethods = new Set(["nineKey", "voice", "handwriting", "unknown", "ai", "reply"]);

/**
 * 统计概览上的「输入方式」：26 键（九宫格以外的所有键盘方案）、9 键、语音和手写。AI 改写、回复和未分类字符不算输入方式，不计入。分组方式与 Android 的 `TypingStatisticsSummary.methods` 一致。
 */
export function summaryMethods(sources: Record<string, number>): BreakdownShare[] {
  const full = Object.keys(sources)
    .filter((key) => !notInputMethods.has(key))
    .reduce((sum, key) => sum + count(sources, key), 0);
  return shares([
    ["26 键", full],
    ["9 键", count(sources, "nineKey")],
    ["语音", count(sources, "voice")],
    ["手写", count(sources, "handwriting")],
  ]);
}
