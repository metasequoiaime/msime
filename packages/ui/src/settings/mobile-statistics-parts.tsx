import type { ReactNode } from "react";
import type { BreakdownShare } from "./typing-breakdown";
import { accentMix } from "./typing-chart";
import { share, shareTotal } from "./typing-summary";
import { shareColour, subText } from "./mobile-statistics-style";
import { MobileStatisticsSectionTitle } from "./mobile-statistics-section-title";

export function Section({
  title,
  trailing,
  children,
}: {
  title: string;
  trailing?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <MobileStatisticsSectionTitle title={title} trailing={trailing} />
      {children}
    </section>
  );
}

type Tile = { label: string; value: string; unit: string; note: string; highlight?: boolean };

/** 两列 KPI 小卡片：一个标签、一个带单位的大数字和一行说明。缺少的数字显示为 "—"，不带单位。 */
export function Tiles({ tiles, card }: { tiles: Tile[]; card: string }) {
  return (
    <div className="grid grid-cols-2 gap-2.5">
      {tiles.map((tile) => {
        const unit = tile.value === "—" ? "" : tile.unit;
        return (
          <div
            className={`${card} flex min-w-0 flex-col gap-1.5 p-3.5`}
            key={tile.label}
            role="group"
            aria-label={`${tile.label} ${tile.value}${unit ? ` ${unit}` : ""}，${tile.note}`}
          >
            <span className={`text-[13px] ${subText}`} aria-hidden="true">
              {tile.label}
            </span>
            <span className="flex items-baseline gap-[3px]" aria-hidden="true">
              <span className="text-[24px] leading-tight font-bold tabular-nums">{tile.value}</span>
              {unit && <span className={`text-[13px] ${subText}`}>{unit}</span>}
            </span>
            <span
              className={`text-[12px] ${tile.highlight ? "font-medium text-[color:var(--p-accent-text)]" : subText}`}
              aria-hidden="true"
            >
              {tile.note}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** 热力图下方的 `少 ▢▢▢▢▢ 多`。 */
export function ShadeLegend({ shades }: { shades: readonly number[] }) {
  return (
    <span className="flex items-center gap-1" aria-hidden="true">
      少
      {shades.map((percentage) => (
        <i
          className="block size-2.5 rounded-[2px]"
          style={{ background: accentMix(percentage) }}
          key={percentage}
        />
      ))}
      多
    </span>
  );
}

/** 图例行：一个圆点、该部分的名称和它的占比。 */
export function ShareLegendRow({
  item,
  index,
  total,
}: {
  item: BreakdownShare;
  index: number;
  total: number;
}) {
  return (
    <div className="flex min-w-0 items-center gap-2 text-[14px]">
      <i
        className="block size-2 shrink-0 rounded-full"
        style={{ background: shareColour(index) }}
        aria-hidden="true"
      />
      <span className="min-w-0 flex-1 truncate">{item.title}</span>
      <span className={`tabular-nums ${subText}`}>{share(item.count, total)}%</span>
    </div>
  );
}

export function shareSummary(items: readonly BreakdownShare[]): string {
  const total = shareTotal(items);
  return items.map((item) => `${item.title} ${share(item.count, total)}%`).join("，");
}
