export type ChartSlice = { count: number; color: string };

/** 环形图的几何参数，单位是 SVG viewBox（0 0 100 100）里的长度。 */
export const DONUT_OUTER = 50;
/** 环宽取外半径的 26%：比原先那道细线粗得多，又给中间的总数留出足够的圆心。 */
export const DONUT_THICKNESS = 13;
export const DONUT_INNER = DONUT_OUTER - DONUT_THICKNESS;
/** 相邻扇区之间留出的底色缝隙，约等于 190px 图上的 2px，两条边平行而不是楔形。 */
const GAP = 1.1;
/** 非零扇区至少占的角度，否则 0.2% 的表情会被缝隙吃掉，图上看不见；多出来的角度按比例从其他扇区扣除。 */
const MIN_SWEEP = 5;

export type DonutSegment = { color: string; d: string } | { color: string; ring: true };

function point(radius: number, degrees: number) {
  const angle = (degrees * Math.PI) / 180;
  return `${(50 + radius * Math.sin(angle)).toFixed(3)} ${(50 - radius * Math.cos(angle)).toFixed(3)}`;
}

/** 按占比分配角度，太小的扇区抬到 MIN_SWEEP；抬高后可能有别的扇区被压到下限以下，所以反复分配直到稳定。 */
function sweeps(counts: readonly number[]): number[] {
  const pinned = new Set<number>();
  for (;;) {
    const free = 360 - pinned.size * MIN_SWEEP;
    const rest = counts.reduce((sum, count, index) => (pinned.has(index) ? sum : sum + count), 0);
    const result = counts.map((count, index) =>
      pinned.has(index) ? MIN_SWEEP : (count / rest) * free,
    );
    const below = result.findIndex((sweep, index) => !pinned.has(index) && sweep < MIN_SWEEP);
    if (below < 0) return result;
    pinned.add(below);
  }
}

/**
 * 环形图的扇区，从 12 点钟方向顺时针排列，顺序与图例一致。只有一个非零类别时画一整圈，不留缝隙。
 */
export function donutSegments(slices: readonly ChartSlice[]): DonutSegment[] {
  const visible = slices.filter((slice) => slice.count > 0);
  if (visible.length === 1) return [{ color: visible[0].color, ring: true }];
  const outerInset = (Math.asin(GAP / 2 / DONUT_OUTER) * 180) / Math.PI;
  const innerInset = (Math.asin(GAP / 2 / DONUT_INNER) * 180) / Math.PI;
  let cursor = 0;
  return sweeps(visible.map((slice) => slice.count)).map((sweep, index) => {
    const start = cursor;
    cursor += sweep;
    const large = sweep - 2 * outerInset > 180 ? 1 : 0;
    const d = [
      `M ${point(DONUT_OUTER, start + outerInset)}`,
      `A ${DONUT_OUTER} ${DONUT_OUTER} 0 ${large} 1 ${point(DONUT_OUTER, cursor - outerInset)}`,
      `L ${point(DONUT_INNER, cursor - innerInset)}`,
      `A ${DONUT_INNER} ${DONUT_INNER} 0 ${large} 0 ${point(DONUT_INNER, start + innerInset)}`,
      "Z",
    ].join(" ");
    return { color: visible[index].color, d };
  });
}

/** 混合到页面卡片颜色上的强调色，即设计稿的 `aM(percent)`：统计摘要的各级色调都由它得出，因而跟随季节强调色。 */
export function accentMix(percent: number, base = "var(--p-group-bg)"): string {
  return `color-mix(in srgb, var(--accent-color) ${percent}%, ${base})`;
}

/**
 * `slices` 的 `conic-gradient` 环形图填充：从 12 点钟方向起按顺序顺时针排列，每块是与其计数成比例的硬边扇区。集合为空时是一整块 `empty` 扇区。
 */
export function conicGradient(slices: readonly ChartSlice[], empty: string): string {
  const total = slices.reduce((sum, slice) => sum + Math.max(0, slice.count), 0);
  if (total <= 0) return `conic-gradient(${empty} 0 100%)`;
  let cursor = 0;
  const stops = slices.map((slice, index) => {
    const start = cursor;
    cursor = index === slices.length - 1 ? 100 : cursor + (Math.max(0, slice.count) / total) * 100;
    return `${slice.color} ${start.toFixed(2)}% ${cursor.toFixed(2)}%`;
  });
  return `conic-gradient(${stops.join(", ")})`;
}
