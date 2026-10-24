import { formatZhNumber } from "../core/format-number";
import { quietBar, subText } from "./mobile-statistics-style";
import { Tiles } from "./mobile-statistics-parts";
import {
  percent,
  speedDelta,
  weekDelta,
  weekdayLabel,
  whole,
  type DayCount,
  type OverviewSummary,
} from "./typing-summary";

/** 头图的七根柱子，今天排在最后并用强调色，每根都标出星期几。 */
function WeekBars({ days }: { days: DayCount[] }) {
  const peak = Math.max(0, ...days.map((day) => day.count));
  const spoken = days
    .map((day) => `星期${weekdayLabel(day.day)} ${formatZhNumber(day.count)} 字`)
    .join("，");
  return (
    <div
      className="flex h-[140px] items-end gap-2.5"
      role="img"
      aria-label={`近 7 天每天的字数：${spoken}`}
    >
      {days.map((day, index) => {
        const today = index === days.length - 1;
        const height = peak <= 0 ? 4 : Math.max(4, Math.round((day.count / peak) * 118));
        return (
          <div
            className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1.5"
            key={day.day}
          >
            <span
              className={`block w-full max-w-[30px] origin-bottom animate-ms-badge-in rounded-[6px] motion-reduce:animate-none ${today ? "bg-[var(--accent-color)]" : quietBar}`}
              style={{ height: `${height}px`, animationDelay: `${index * 40}ms` }}
            />
            <span
              className={`text-[11px] whitespace-nowrap ${today ? "text-[color:var(--p-text)]" : subText}`}
            >
              {weekdayLabel(day.day)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export function MobileStatisticsOverview({
  overview,
  card,
}: {
  overview: OverviewSummary;
  card: string;
}) {
  const delta = weekDelta(overview.week_total, overview.previous_week_total);
  const speedNote = speedDelta(overview.average_speed, overview.previous_average_speed);
  return (
    <>
      <section
        className={`${card} flex flex-col gap-[18px] px-4 pt-[18px] pb-3.5`}
        aria-label="近 7 天概览"
      >
        <div className="flex flex-col gap-1">
          <span className={`text-[13px] ${subText}`}>近 7 天共输入</span>
          <span className="flex items-baseline gap-1.5">
            <span className="text-[40px] leading-[1.1] font-bold tracking-[-0.02em] tabular-nums">
              {formatZhNumber(overview.week_total)}
            </span>
            <span className={`text-[15px] ${subText}`}>字</span>
          </span>
          <span
            className={`text-[13px] font-semibold ${delta ? "text-[color:var(--p-accent-text)]" : subText}`}
          >
            {delta ?? "—"}
          </span>
        </div>
        <WeekBars days={overview.last7} />
      </section>
      <Tiles
        card={card}
        tiles={[
          {
            label: "平均速度",
            value: whole(overview.average_speed),
            unit: "字/分",
            note: speedNote ?? "近 7 天的活跃时间里",
          },
          {
            label: "首选命中",
            value: percent(overview.first_candidate_rate),
            unit: "%",
            note:
              overview.first_candidate_rate === null
                ? "选词满 50 次后显示"
                : "第一个候选就是你要的",
          },
          {
            label: "少按键",
            value: percent(overview.keystrokes_saved_rate),
            unit: "%",
            note: "联想和整句帮你省下",
          },
          {
            label: "连续使用",
            value: String(overview.current_streak),
            unit: "天",
            note: `最长 ${overview.longest_streak} 天`,
          },
        ]}
      />
    </>
  );
}
