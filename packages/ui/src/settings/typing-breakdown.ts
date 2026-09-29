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
