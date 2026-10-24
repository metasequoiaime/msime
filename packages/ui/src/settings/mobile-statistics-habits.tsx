import { formatZhNumber } from "../core/format-number";
import { summaryComposition } from "./typing-breakdown";
import { accentMix } from "./typing-chart";
import { summaryHeatLevel } from "./keyboard-heatmap";
import { Section, ShadeLegend, ShareLegendRow, shareSummary } from "./mobile-statistics-parts";
import { quietBar, shareColour, subText } from "./mobile-statistics-style";
import { peakLabel, shareTotal, type HabitsSummary } from "./typing-summary";

/** 日历的五档颜色，即设计稿的 `heatC`。 */
const dayShades = [6, 18, 38, 62, 90];

const heatmapCells = 84;

export function MobileStatisticsHabits({ habits, card }: { habits: HabitsSummary; card: string }) {
  // 不足 84 天时在前面补齐，让今天总是落在右下角。
  const days = habits.weeks12.slice(-heatmapCells);
  const cells = [
    ...Array.from({ length: heatmapCells - days.length }, () => 0),
    ...days.map((day) => day.count),
  ];
  const dayPeak = Math.max(0, ...cells);
  const hours = Array.from({ length: 24 }, (_, hour) => habits.hours24[hour] ?? 0);
  const hourPeak = Math.max(0, ...hours);
  const peak = peakLabel(habits.peak_window);
  const composition = summaryComposition(habits.breakdown.characters);
  const compositionTotal = shareTotal(composition);
  return (
    <>
      <Section
        title="近 12 周"
        trailing={<span className={`text-[12px] ${subText}`}>活跃 {habits.active_days} 天</span>}
      >
        <div className={`${card} flex flex-col gap-2.5 p-3.5`}>
          <div
            className="grid grid-flow-col grid-cols-12 grid-rows-[repeat(7,auto)] gap-[3px]"
            role="img"
            aria-label={`近 12 周输入热力图，${habits.active_days} 天有输入，最多一天 ${formatZhNumber(dayPeak)} 字`}
          >
            {cells.map((count, index) => (
              <span
                className="block aspect-square rounded-[3px]"
                style={{ background: accentMix(dayShades[summaryHeatLevel(count, dayPeak)]) }}
                key={index}
              />
            ))}
          </div>
          <div className={`flex justify-end text-[11px] ${subText}`}>
            <ShadeLegend shades={dayShades} />
          </div>
        </div>
      </Section>
      <Section
        title="活跃时段"
        trailing={peak && <span className={`text-[12px] ${subText}`}>最常在 {peak}</span>}
      >
        <div className={`${card} flex flex-col gap-1.5 px-3.5 pt-3.5 pb-2.5`}>
          <div
            className="flex h-14 items-end gap-0.5"
            role="img"
            aria-label={
              peak ? `24 小时输入分布，最常在${peak}` : "24 小时输入分布，近 7 天还没有记录"
            }
          >
            {hours.map((count, hour) => {
              const height = hourPeak <= 0 ? 3 : Math.max(3, Math.round((count / hourPeak) * 56));
              const busy = hourPeak > 0 && count >= (hourPeak * 52) / 60;
              return (
                <span
                  className={`block min-w-0 flex-1 rounded-[2px] ${busy ? "bg-[var(--accent-color)]" : quietBar}`}
                  style={{ height: `${height}px` }}
                  key={hour}
                />
              );
            })}
          </div>
          <div className={`flex justify-between text-[11px] ${subText}`} aria-hidden="true">
            <span>0 时</span>
            <span>6</span>
            <span>12</span>
            <span>18</span>
            <span>24</span>
          </div>
        </div>
      </Section>
      <Section title="输入构成">
        <div className={`${card} flex flex-col gap-3.5 p-4`}>
          {composition.length === 0 ? (
            <span className={`text-[14px] ${subText}`}>还没有记录</span>
          ) : (
            <>
              <div
                className="flex h-3 gap-0.5 overflow-hidden rounded-[6px]"
                role="img"
                aria-label={`输入构成：${shareSummary(composition)}`}
              >
                {composition.map((item, index) => (
                  <span
                    className="block h-full min-w-0.5"
                    style={{ flex: `${item.count} 1 0%`, background: shareColour(index) }}
                    key={item.title}
                  />
                ))}
              </div>
              <div className="grid grid-cols-2 gap-x-4 gap-y-2.5" aria-hidden="true">
                {composition.map((item, index) => (
                  <ShareLegendRow
                    item={item}
                    index={index}
                    total={compositionTotal}
                    key={item.title}
                  />
                ))}
              </div>
            </>
          )}
        </div>
      </Section>
    </>
  );
}
