export type Identified = { id: string };

/** Append only items whose ids are not already present, preserving source order. */
export function appendUniqueById<T extends Identified>(current: T[], incoming: T[]): T[] {
  const ids = new Set(current.map((item) => item.id));
  const additions = incoming.filter((item) => {
    if (ids.has(item.id)) return false;
    ids.add(item.id);
    return true;
  });
  return [...current, ...additions];
}

export function communityRating(count: number, average: number): string {
  return count === 0 ? "暂无评分" : `${average.toFixed(1)} 分`;
}
