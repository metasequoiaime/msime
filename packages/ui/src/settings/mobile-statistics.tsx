import { useId, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import { FluentIcon } from "../core/fluent-icons";
import { formatZhNumber } from "../core/format-number";
import {
  keyLabel,
  phoneHeatmapLeader,
  phoneHeatmapPeak,
  phoneHeatmapRows,
  prefersNineKey,
  scopedKeyCounts,
  summaryHeatLevel,
  type DailyKeyCounts,
  type PhoneHeatmapLayout,
} from "./keyboard-heatmap";
import { StatisticsBadges } from "./statistics-badges";
import { summaryComposition, summaryMethods, type BreakdownShare } from "./typing-breakdown";
import { accentMix, conicGradient } from "./typing-chart";
import {
  decimal,
  monthDay,
  peakLabel,
  percent,
  percentTenths,
  perKeyDelta,
  share,
  shareTotal,
  speedDelta,
  weekDelta,
  weekdayLabel,
  whole,
  type DayCount,
  type HabitsSummary,
  type KeysSummary,
  type OverviewSummary,
  type TypingSummary,
} from "./typing-summary";

/** 绘制由摘要驱动的统计页的两种 HarmonyOS 外观：手机，以及内容相同、卡片圆角更小的 2-in-1。 */
export type StatisticsLook = "harmony" | "hm2";

type StatisticsTab = "overview" | "habits" | "keys" | "achievements";

const tabs: [StatisticsTab, string][] = [
  ["overview", "概览"],
  ["habits", "习惯"],
  ["keys", "按键"],
  ["achievements", "成就"],
];

// ---- 共用部件 ----

const subText = "text-[color:var(--p-sub)]";
/** 设计稿的 `stSegBg`：浅色模式下是卡片上一层淡淡的强调色，深色模式下是半透明的白色。 */
const segmentTrack =
  "bg-[rgba(255,255,255,0.08)] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_9%,var(--p-group-bg))]";
const segmentOn = "bg-[#636366] font-semibold text-[color:var(--p-text)] light-theme:bg-white";
/** 进度条的空白部分，即设计稿的 `stTrack`。 */
const trackClass = "bg-[rgba(255,255,255,0.1)] light-theme:bg-[rgba(0,0,0,0.07)]";
/** 非高亮的柱子：浅色模式下是卡片上 18% 的强调色，深色模式下是 24%（设计稿的 `aM(dark ? 24 : 18)`）。 */
const quietBar =
  "bg-[color-mix(in_srgb,var(--accent-color)_24%,var(--p-group-bg))] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_18%,var(--p-group-bg))]";

/** 日历的五档颜色，即设计稿的 `heatC`。 */
const dayShades = [6, 18, 38, 62, 90];
/** 按键的五档颜色，每档都比日历的略浅，让最高一档上的白字仍然清晰可读。 */
const keyShades = [6, 16, 32, 56, 88];
/** 占比图的颜色，按部分的先后排列；第四个之后的部分沿用最后一种。 */
const shareShades = [100, 52, 28, 15];
/** 候选位置，从第一位到翻页之后。 */
const positionShades = [100, 60, 40, 25];

const shade = (percentage: number) =>
  percentage >= 100 ? "var(--accent-color)" : accentMix(percentage);
const shareColour = (index: number) => shade(shareShades[Math.min(index, shareShades.length - 1)]);

/** `look` 外观下的统计卡片：分组底色，手机上圆角 20px、2-in-1 上 12px，没有边框和阴影。 */
export function statisticsCardClass(look: StatisticsLook): string {
  return `bg-[var(--p-group-bg)] ${look === "hm2" ? "rounded-[12px]" : "rounded-[20px]"}`;
}

/** 卡片上方的小标题，末端可带一条备注。 */
function SectionTitle({ title, trailing }: { title: string; trailing?: ReactNode }) {
  return (
    <div className="flex min-h-5 items-center justify-between gap-3 px-1">
      <h3 className={`m-0 text-[13px] font-normal ${subText}`}>{title}</h3>
      {trailing}
    </div>
  );
}

function Section({
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
      <SectionTitle title={title} trailing={trailing} />
      {children}
    </section>
  );
}

type Tile = { label: string; value: string; unit: string; note: string; highlight?: boolean };

/** 两列 KPI 小卡片：一个标签、一个带单位的大数字和一行说明。缺少的数字显示为 "—"，不带单位。 */
function Tiles({ tiles, card }: { tiles: Tile[]; card: string }) {
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
function ShadeLegend({ shades }: { shades: readonly number[] }) {
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
function ShareLegendRow({
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

function shareSummary(items: readonly BreakdownShare[]): string {
  const total = shareTotal(items);
  return items.map((item) => `${item.title} ${share(item.count, total)}%`).join("，");
}

// ---- 概览 ----

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

function OverviewPanel({ overview, card }: { overview: OverviewSummary; card: string }) {
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

// ---- 习惯 ----

const heatmapCells = 84;

function HabitsPanel({ habits, card }: { habits: HabitsSummary; card: string }) {
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

// ---- 按键 ----

/** 按键热力图标题旁的 26 键 | 9 键切换。 */
function LayoutSwitch({
  value,
  onChange,
}: {
  value: PhoneHeatmapLayout;
  onChange: (value: PhoneHeatmapLayout) => void;
}) {
  return (
    <div
      className={`flex rounded-[8px] p-0.5 ${segmentTrack}`}
      role="group"
      aria-label="热力图键盘"
    >
      {(
        [
          ["full", "26 键"],
          ["nine", "9 键"],
        ] as const
      ).map(([layout, label]) => {
        const on = value === layout;
        return (
          <button
            type="button"
            className={`flex h-6 items-center rounded-[6px] border-0 px-2.5 text-[12px] ${on ? `${segmentOn} shadow-[0_1px_2px_rgba(0,0,0,0.12)]` : "bg-transparent text-[color:var(--p-text)]"}`}
            aria-pressed={on}
            onClick={() => onChange(layout)}
            key={layout}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}

/** 每个键在 flex 空间里的份额：26 键键盘按设计稿的排法以一行的十分之一为单位定宽，所以较短的行会居中；九键网格的键只按权重平分所在的行。 */
function keyFlex(layout: PhoneHeatmapLayout, weight: number): CSSProperties {
  return layout === "nine"
    ? { flex: `${weight} 1 0%` }
    : { flex: `0 0 calc((100% - 36px) / 10 * ${weight} + ${(weight - 1) * 4}px)` };
}

/**
 * 用户所用手机键盘最近七天的按键热力图：每个键按它的次数相对于所画按键中最忙的那个着色，并标出它占全部按键次数的比例，键盘下方给出按得最多的拼写键。
 */
function KeyHeatmap({ counts, card }: { counts: Record<string, number>; card: string }) {
  const [chosen, setChosen] = useState<PhoneHeatmapLayout | null>(null);
  const layout: PhoneHeatmapLayout = chosen ?? (prefersNineKey(counts) ? "nine" : "full");
  const total = Object.values(counts).reduce((sum, count) => sum + count, 0);
  const peak = phoneHeatmapPeak(counts, layout);
  const leader = phoneHeatmapLeader(counts, layout);
  const headline = leader
    ? `${layout === "nine" ? "最常按" : "字母键最常按"} ${leader.label} · ${percentTenths(leader.count / total)}%`
    : "这段时间还没有按键记录";
  let order = 0;
  return (
    <Section title="按键热力图" trailing={<LayoutSwitch value={layout} onChange={setChosen} />}>
      <div className={`${card} flex flex-col gap-2.5 px-1 py-2.5`}>
        <div
          className="flex flex-col gap-1.5"
          role="group"
          aria-label={`${layout === "nine" ? "9 键" : "26 键"}按键热力图，${headline}`}
        >
          {phoneHeatmapRows[layout].map((row, rowIndex) => (
            <div className="flex justify-center gap-1" key={`${layout}-${rowIndex}`}>
              {row.map((key) => {
                const code = key.code ?? "";
                const count = counts[code] ?? 0;
                const level = summaryHeatLevel(count, peak);
                const delay = order++ * 18;
                return (
                  <span
                    className={`flex min-w-0 animate-ms-badge-in flex-col items-center justify-center gap-px overflow-hidden rounded-[6px] [animation-duration:0.4s] motion-reduce:animate-none ${layout === "nine" ? "h-[52px]" : "h-11"} ${level >= 4 ? "text-[color:var(--p-on-accent,#fff)]" : "text-[color:var(--p-text)]"}`}
                    style={{
                      ...keyFlex(layout, key.weight),
                      background: accentMix(keyShades[level]),
                      animationDelay: `${delay}ms`,
                    }}
                    role="img"
                    aria-label={`${keyLabel(code)}，${formatZhNumber(count)} 次`}
                    key={code}
                  >
                    <span
                      className={`leading-none font-semibold whitespace-nowrap ${layout === "nine" ? "text-[15px]" : "text-[14px]"}`}
                      aria-hidden="true"
                    >
                      {key.label}
                    </span>
                    {total > 0 && (
                      <span className="text-[9px] leading-none opacity-85" aria-hidden="true">
                        {percentTenths(count / total)}%
                      </span>
                    )}
                  </span>
                );
              })}
            </div>
          ))}
        </div>
        <div className={`flex items-center justify-between gap-3 px-1 text-[12px] ${subText}`}>
          <span className="min-w-0 truncate">{headline}</span>
          <ShadeLegend shades={keyShades} />
        </div>
      </div>
    </Section>
  );
}

function KeysPanel({
  keys,
  habits,
  counts,
  card,
}: {
  keys: KeysSummary;
  habits: HabitsSummary;
  counts: Record<string, number>;
  card: string;
}) {
  const perKeyNote = perKeyDelta(keys.per_character_keys, keys.previous_per_character_keys);
  const run = keys.longest_run;
  const methods = summaryMethods(habits.breakdown.sources);
  const methodsTotal = shareTotal(methods);
  const positions = keys.positions;
  return (
    <>
      <KeyHeatmap counts={counts} card={card} />
      <Tiles
        card={card}
        tiles={[
          {
            label: "每字按键",
            value: decimal(keys.per_character_keys),
            unit: "次",
            note: perKeyNote ?? "近 7 天平均",
            highlight: perKeyNote !== null,
          },
          {
            label: "退格占比",
            value: percentTenths(keys.backspace_rate),
            unit: "%",
            note: "近 7 天全部按键里",
          },
          {
            label: "联想上屏",
            value: percent(keys.prediction_rate),
            unit: "%",
            note: "不用打完就上屏的词",
          },
          {
            label: "单次最长",
            value: run ? String(run.characters) : "—",
            unit: "字",
            note: run ? `${monthDay(run.day)} · 不停顿` : "还没有记录",
          },
        ]}
      />
      {positions && (
        <Section title="选词位置">
          <div className={`${card} flex flex-col gap-3 p-4`}>
            {(["第 1 个", "第 2 个", "第 3 个", "翻页后"] as const).map((label, index) => {
              const value = `${Math.round((positions[index] ?? 0) * 100)}%`;
              return (
                <div
                  className="flex items-center gap-3 text-[14px]"
                  key={label}
                  role="group"
                  aria-label={`${label} ${value}`}
                >
                  <span className="w-[54px] flex-none" aria-hidden="true">
                    {label}
                  </span>
                  <span
                    className={`block h-2.5 flex-1 overflow-hidden rounded-[5px] ${trackClass}`}
                    aria-hidden="true"
                  >
                    <span
                      className="block h-full animate-ms-bar-fill rounded-[5px] [animation-duration:0.9s] motion-reduce:animate-none"
                      style={{
                        width: value,
                        background: shade(positionShades[index]),
                        animationDelay: `${index * 80}ms`,
                      }}
                    />
                  </span>
                  <span className={`w-10 text-right tabular-nums ${subText}`} aria-hidden="true">
                    {value}
                  </span>
                </div>
              );
            })}
          </div>
        </Section>
      )}
      {methods.length > 0 && (
        <Section title="输入方式">
          <div className={`${card} flex items-center gap-[18px] p-4`}>
            <div
              className="flex size-24 flex-none items-center justify-center rounded-full"
              style={{
                background: conicGradient(
                  methods.map((item, index) => ({ count: item.count, color: shareColour(index) })),
                  "transparent",
                ),
              }}
              role="img"
              aria-label={`输入方式：${shareSummary(methods)}`}
            >
              <div
                className="flex size-[66px] flex-col items-center justify-center rounded-full bg-[var(--p-group-bg)]"
                aria-hidden="true"
              >
                <span className="text-[17px] font-bold tabular-nums">
                  {share(methods[0].count, methodsTotal)}%
                </span>
                <span className={`text-[10px] ${subText}`}>{methods[0].title}</span>
              </div>
            </div>
            <div className="flex min-w-0 flex-1 flex-col gap-2.5" aria-hidden="true">
              {methods.map((item, index) => (
                <ShareLegendRow item={item} index={index} total={methodsTotal} key={item.title} />
              ))}
            </div>
          </div>
        </Section>
      )}
    </>
  );
}

// ---- 页面 ----

/** 每个子标签页底部的那一行隐私说明。 */
function StatisticsFooter() {
  return (
    <p className={`m-0 flex items-center justify-center gap-1.5 text-[12px] ${subText}`}>
      <FluentIcon name="shield_lock" size={13} />
      统计只保存在本机，不包含输入内容
    </p>
  );
}

/**
 * HarmonyOS 的 统计 标签页：分段控件 概览 / 习惯 / 按键 / 成就，下面是由共享的 `summary` 绘制的卡片（数字都在 Rust 里算好，这里只负责排版），最后是一行隐私说明。对应 Android 的 `StatisticsFragment`；和它一样没有 常用词 和 最常打错 卡片，因为统计从不保存输入过的内容。
 *
 * `dailyKeys` 和 Android 一样取自统计文档而不是摘要：按键热力图覆盖摘要的最近七天。
 */
export function MobileStatistics({
  look,
  summary,
  dailyKeys,
}: {
  look: StatisticsLook;
  summary: TypingSummary;
  dailyKeys: DailyKeyCounts | undefined;
}) {
  const [tab, setTab] = useState<StatisticsTab>("overview");
  const id = useId();
  const card = statisticsCardClass(look);
  // 把时间窗口拼成一个字符串，这样只有窗口移动或计数变化时才重新求和。
  const weekKey = summary.overview.last7.map((day) => day.day).join(",");
  const keyCounts = useMemo(
    () => scopedKeyCounts(dailyKeys, weekKey ? weekKey.split(",") : []),
    [dailyKeys, weekKey],
  );
  return (
    <>
      <div
        className={`flex rounded-[9px] p-0.5 ${segmentTrack}`}
        role="tablist"
        aria-label="统计内容"
      >
        {tabs.map(([value, label]) => {
          const on = tab === value;
          return (
            <button
              type="button"
              role="tab"
              id={`${id}-${value}-tab`}
              aria-controls={`${id}-${value}`}
              aria-selected={on}
              className={`flex h-8 flex-1 items-center justify-center rounded-[7px] border-0 text-[14px] ${on ? `${segmentOn} shadow-[0_1px_3px_rgba(0,0,0,0.12)]` : "bg-transparent text-[color:var(--p-text)]"}`}
              onClick={() => setTab(value)}
              key={value}
            >
              {label}
            </button>
          );
        })}
      </div>
      <div
        className="flex flex-col gap-[22px]"
        role="tabpanel"
        id={`${id}-${tab}`}
        aria-labelledby={`${id}-${tab}-tab`}
      >
        {tab === "overview" && <OverviewPanel overview={summary.overview} card={card} />}
        {tab === "habits" && <HabitsPanel habits={summary.habits} card={card} />}
        {tab === "keys" && (
          <KeysPanel keys={summary.keys} habits={summary.habits} counts={keyCounts} card={card} />
        )}
        {tab === "achievements" && (
          <StatisticsBadges achievements={summary.achievements} card={card} />
        )}
      </div>
      <StatisticsFooter />
    </>
  );
}
