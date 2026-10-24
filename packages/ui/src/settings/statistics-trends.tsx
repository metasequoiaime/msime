import { formatZhNumber } from "../core/format-number";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { StatisticsNote } from "./statistics-note";
import { StatisticsChartButton } from "./statistics-chart-button";

const bar = "block w-full min-h-0.5 rounded-t-[3px] rounded-b-[1px]";
const axis = "mt-[7px] flex justify-between text-xs text-muted";

/** 每日趋势的两种外观共用峰值、日均和坐标轴；选择状态由页面持有。 */
export function StatisticsDailyTrend({
  days,
  counts,
  selectedDay,
  onSelect,
  mobile = false,
}: {
  days: { key: string; label: string }[];
  counts: Record<string, number>;
  selectedDay: string | null;
  onSelect: (key: string) => void;
  mobile?: boolean;
}) {
  const maximum = Math.max(1, ...days.map((day) => counts[day.key] ?? 0));
  // 日均仅统计窗口内有输入的日期，与节奏卡片相同，空白日不拉低参考线。
  const recorded = days.map((day) => counts[day.key] ?? 0).filter((count) => count > 0);
  const average =
    recorded.length === 0 ? 0 : recorded.reduce((sum, count) => sum + count, 0) / recorded.length;
  return (
    <>
      <StatisticsNote>
        最高 {maximum === 1 && days.every((day) => !counts[day.key]) ? 0 : formatZhNumber(maximum)}{" "}
        字符 / 天{average > 0 && ` · 虚线为日均 ${formatZhNumber(Math.round(average))} 字符`}
      </StatisticsNote>
      {mobile ? (
        <StatisticsTrendLine
          days={days}
          counts={counts}
          selectedDay={selectedDay}
          average={average}
        />
      ) : (
        <div
          className={`relative mt-3 flex h-[145px] items-end ${days.length === 7 ? "gap-2.5" : "gap-[3px]"} max-phone:gap-0.5`}
        >
          {days.map((day) => {
            const count = counts[day.key] ?? 0;
            const chosen = selectedDay === day.key;
            // 根据受控选择直接调暗其余柱子。
            const dimmed = selectedDay !== null && !chosen;
            return (
              <StatisticsChartButton
                className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1 border-0 bg-transparent p-0 text-[10px] text-muted"
                key={day.key}
                title={`${day.label}：${count} 字符`}
                ariaLabel={`${day.label}，${count} 字符`}
                selected={chosen}
                onClick={() => onSelect(day.key)}
              >
                {days.length === 7 && <span>{count}</span>}
                <i
                  className={`${bar} ${chosen ? "bg-[#e59b43]" : "bg-accent"} ${dimmed ? "opacity-40" : chosen ? "opacity-100" : "opacity-85"}`}
                  style={{ height: `${Math.max(2, (count / maximum) * 100)}%` }}
                />
              </StatisticsChartButton>
            );
          })}
          {average > 0 && <ReferenceLine bottom={(average / maximum) * 100} />}
        </div>
      )}
      <div className={axis}>
        <span>{days[0]?.label}</span>
        <span>{days.at(-1)?.label}</span>
      </div>
    </>
  );
}

/** 列中心在 0 到 100 的 viewBox 中的位置，让覆盖在柱图上的折线穿过柱子中心。 */
const columnCenter = (index: number, count: number) => ((index + 0.5) / Math.max(1, count)) * 100;

/** 遇到 null 就抬笔，让未测量日显示为断线，而不是下降到零。 */
function linePath(values: readonly (number | null)[], y: (value: number) => number): string {
  let path = "";
  let drawing = false;
  values.forEach((value, index) => {
    if (value === null) {
      drawing = false;
      return;
    }
    path += `${drawing ? "L" : "M"}${columnCenter(index, values.length)},${y(value)} `;
    drawing = true;
  });
  return path.trim();
}

/** 虚线表示参考阈值；数值放在图上方的说明里，避免标签挡住数据。 */
function ReferenceLine({ bottom }: { bottom: number }) {
  return (
    <i
      className="pointer-events-none absolute inset-x-0 block h-px bg-[repeating-linear-gradient(90deg,var(--text-muted)_0_4px,transparent_4px_8px)] opacity-70"
      style={{ bottom: `${bottom}%` }}
      aria-hidden="true"
    />
  );
}

/** 每日活跃分钟速度用折线展示，纵轴从最低速度附近起算并标出两端；测量不足的日期留空。 */
export function SpeedTrend({
  days,
  speeds,
  averageSpeed,
  selectedDay,
}: {
  days: { key: string; label: string }[];
  speeds: (number | null)[];
  averageSpeed: number;
  selectedDay: string | null;
}) {
  const known = speeds.filter((speed): speed is number => speed !== null);
  const count = (value: number) => formatZhNumber(Math.round(value));
  if (known.length === 0)
    return (
      <SettingsEmptyMessage centered className="mt-3.5">
        这段时间还没有测量到足够的活跃时长。每天连续打字满 1 分钟后，这里会画出当天的速度。
      </SettingsEmptyMessage>
    );
  const high = Math.max(...known, averageSpeed);
  const low = Math.min(...known, averageSpeed);
  const spread = Math.max(high - low, high * 0.2, 1);
  const top = Math.ceil(high + spread * 0.15);
  const bottom = Math.max(0, Math.floor(low - spread * 0.25));
  const height = (value: number) => ((value - bottom) / Math.max(1, top - bottom)) * 100;
  const y = (value: number) => 100 - height(value);
  // 间距足够时显示每个圆点；长范围只显示孤立点，因为它没有相邻折线可供辨认。
  const dotted = (index: number) =>
    speeds[index] !== null &&
    (days.length <= 45 || (speeds[index - 1] == null && speeds[index + 1] == null));
  return (
    <>
      <StatisticsNote>
        最快 {count(Math.max(...known))} · 最慢 {count(Math.min(...known))} · 虚线为平均{" "}
        {count(averageSpeed)} 字 / 分钟 · {known.length} 天有测量
      </StatisticsNote>
      <div
        className="relative mt-3 h-[120px] border-b border-edge text-accent"
        role="img"
        aria-label={`每日输入速度折线图，${known.length} 天有测量，平均 ${count(averageSpeed)} 字每分钟`}
      >
        {/* 纵轴不从零起算，在图内标出上下边界，保持与每日柱图的列对齐。 */}
        <span
          className="pointer-events-none absolute top-0 left-0 text-[10px] leading-none text-muted tabular-nums"
          aria-hidden="true"
        >
          {top}
        </span>
        <span
          className="pointer-events-none absolute bottom-1 left-0 text-[10px] leading-none text-muted tabular-nums"
          aria-hidden="true"
        >
          {bottom}
        </span>
        <ReferenceLine bottom={height(averageSpeed)} />
        <svg
          className="absolute inset-0 block size-full overflow-visible"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <path
            d={linePath(speeds, y)}
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        {/* 圆点和悬浮列用 HTML 绘制，避免 SVG 随容器拉伸时把圆压成椭圆。 */}
        {days.map((day, index) => {
          const speed = speeds[index];
          const chosen = selectedDay === day.key;
          return (
            <span
              className="absolute inset-y-0"
              style={{
                left: `${(index / days.length) * 100}%`,
                width: `${100 / days.length}%`,
              }}
              key={day.key}
              title={`${day.label}：${speed === null ? "活跃不足 1 分钟，未计算" : `${count(speed)} 字 / 分钟`}`}
              aria-hidden="true"
            >
              {speed !== null && (dotted(index) || chosen) && (
                <i
                  className={`absolute left-1/2 block -translate-x-1/2 translate-y-1/2 rounded-full ring-2 ring-card-solid ${chosen ? "size-2.5 bg-[#e59b43]" : "size-[7px] bg-accent"}`}
                  style={{ bottom: `${height(speed)}%` }}
                />
              )}
            </span>
          );
        })}
      </div>
      <div className={axis}>
        <span>{days[0]?.label}</span>
        <span>{days.at(-1)?.label}</span>
      </div>
    </>
  );
}

function StatisticsTrendLine({
  days,
  counts,
  selectedDay,
  average,
}: {
  days: { key: string; label: string }[];
  counts: Record<string, number>;
  selectedDay: string | null;
  average: number;
}) {
  const values = days.map((day) => counts[day.key] ?? 0);
  const lineValues =
    days.length > 120
      ? values.map((_, index) => {
          const start = Math.max(0, index - 6);
          const window = values.slice(start, index + 1);
          return window.reduce((total, value) => total + value, 0) / window.length;
        })
      : values;
  const maximum = Math.max(1, ...values, ...lineValues);
  const point = (value: number, index: number) =>
    `${(index / Math.max(1, days.length - 1)) * 100},${96 - (value / maximum) * 88}`;
  const line = lineValues.map(point).join(" ");
  const area = `0,100 ${values.map(point).join(" ")} 100,100`;
  const selectedIndex = selectedDay ? days.findIndex((day) => day.key === selectedDay) : -1;
  const selectedValue = selectedIndex >= 0 ? values[selectedIndex] : 0;
  return (
    <div
      className="relative mt-3 h-[170px] w-full text-accent"
      role="img"
      aria-label={days.length > 120 ? "每日输入趋势折线图，显示七日均线" : "每日输入趋势折线图"}
    >
      {average > 0 && <ReferenceLine bottom={4 + (average / maximum) * 88} />}
      <svg
        className="block size-full overflow-visible"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <defs>
          <linearGradient id="statistics-trend-area" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="currentColor" stopOpacity=".34" />
            <stop offset="1" stopColor="currentColor" stopOpacity=".03" />
          </linearGradient>
        </defs>
        <polygon points={area} fill="url(#statistics-trend-area)" />
        <polyline
          points={line}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          vectorEffect="non-scaling-stroke"
        />
        {selectedIndex >= 0 && (
          <circle
            className="fill-[#e59b43] stroke-card stroke-[1.5px]"
            cx={(selectedIndex / Math.max(1, days.length - 1)) * 100}
            cy={96 - (selectedValue / maximum) * 88}
            r="2.2"
            vectorEffect="non-scaling-stroke"
          />
        )}
      </svg>
    </div>
  );
}

/** 保留完整小时列；今日用强调色柱图，历史均值用中性色折线，二者共用量程。 */
export function StatisticsHourlyBars({
  hours,
  usual,
}: {
  hours: readonly number[];
  usual: { hours: number[]; days: number } | null;
}) {
  const peak = Math.max(1, ...hours, ...(usual?.hours ?? []));
  const total = hours.reduce((sum, count) => sum + count, 0);
  const count = (value: number) => formatZhNumber(Math.round(value));
  return (
    <>
      <StatisticsNote>
        最高 {count(Math.max(0, ...hours))} 字符 / 小时 · 共 {count(total)} 字符
      </StatisticsNote>
      {usual && (
        <div className="mt-2 flex items-center gap-4 text-[11px] text-secondary" aria-hidden="true">
          <span className="flex items-center gap-1.5">
            <i className="block size-2.5 rounded-[2px] bg-accent" />
            今日
          </span>
          <span className="flex items-center gap-1.5">
            <i className="block h-0.5 w-3.5 rounded-full bg-[var(--text-secondary)]" />
            平时（前 {formatZhNumber(usual.days)} 天平均）
          </span>
        </div>
      )}
      <div
        className="relative mt-3 flex h-[110px] items-end gap-[3px]"
        role="img"
        aria-label={usual ? "今日各时段输入分布，与平时对比" : "今日各时段输入分布"}
      >
        {hours.map((value, hour) => (
          <div
            key={hour}
            className="flex h-full min-w-0 flex-1 flex-col justify-end"
            title={`${hour} 时：今日 ${value} 字符${usual ? `，平时 ${count(usual.hours[hour])} 字符` : ""}`}
            aria-label={`${hour} 时，${value} 字符`}
          >
            <i
              className={`${bar} bg-accent ${value === 0 ? "opacity-25" : "opacity-85"}`}
              style={{ height: `${Math.max(2, (value / peak) * 100)}%` }}
            />
          </div>
        ))}
        {usual && (
          <svg
            className="pointer-events-none absolute inset-0 block size-full overflow-visible text-[var(--text-secondary)]"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path
              d={linePath(usual.hours, (value) => 100 - (value / peak) * 100)}
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        )}
      </div>
      <div className={axis}>
        <span>0 时</span>
        <span>12 时</span>
        <span>23 时</span>
      </div>
    </>
  );
}
