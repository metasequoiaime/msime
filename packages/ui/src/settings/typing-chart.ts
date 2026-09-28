export type ChartSlice = { count: number; color: string };

export function chartGradient(slices: readonly ChartSlice[], total: number): string {
  let cursor = 0;
  const segments = slices
    .filter((slice) => slice.count > 0)
    .map((slice) => {
      const start = (cursor / total) * 360;
      cursor += slice.count;
      return `${slice.color} ${start}deg ${(cursor / total) * 360}deg`;
    });
  return segments.length ? `conic-gradient(${segments.join(", ")})` : "var(--surface-subtle)";
}
