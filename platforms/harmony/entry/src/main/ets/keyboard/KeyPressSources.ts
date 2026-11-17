/** 按下时固定的来源聚合；只保留各来源的计数，不保留切换顺序。 */
export class KeyPressSources {
  private counts: Map<string, Map<string, number>> = new Map<string, Map<string, number>>();

  add(id: string, source: string): void {
    let keys: Map<string, number> | undefined = this.counts.get(source);
    if (keys === undefined) {
      keys = new Map<string, number>();
      this.counts.set(source, keys);
    }
    keys.set(id, (keys.get(id) ?? 0) + 1);
  }

  snapshot(): Record<string, Record<string, number>> {
    const sources: Record<string, Record<string, number>> = {};
    this.counts.forEach((keys: Map<string, number>, source: string): void => {
      const values: Record<string, number> = {};
      keys.forEach((count: number, id: string): void => {
        values[id] = count;
      });
      sources[source] = values;
    });
    return sources;
  }

  clear(): void {
    this.counts.clear();
  }
}
