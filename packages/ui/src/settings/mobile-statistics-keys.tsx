import { useId, useState, type CSSProperties } from "react";
import { formatZhNumber } from "../core/format-number";
import { statisticsTrack } from "../core/statistics-style";
import {
  keyLabel,
  phoneHeatmapCount,
  phoneHeatmapLeader,
  phoneHeatmapOthers,
  phoneHeatmapPeak,
  phoneHeatmapRows,
  phoneHeatmapSharedCodes,
  prefersNineKey,
  summaryHeatLevel,
  type PhoneHeatmapLayout,
} from "./keyboard-heatmap";
import {
  Section,
  Tiles,
  ShadeLegend,
  ShareLegendRow,
  shareSummary,
} from "./mobile-statistics-parts";
import { segmentTrack, segmentOn, shade, shareColour, subText } from "./mobile-statistics-style";
import { summaryMethods } from "./typing-breakdown";
import { accentMix, conicGradient } from "./typing-chart";
import {
  decimal,
  monthDay,
  percent,
  percentTenths,
  perKeyDelta,
  share,
  shareTotal,
  type HabitsSummary,
  type KeysSummary,
} from "./typing-summary";

/** 按键的五档颜色，每档都比日历的略浅，让最高一档上的白字仍然清晰可读。 */
const keyShades = [6, 16, 32, 56, 88];

/** 候选位置，从第一位到翻页之后。 */
const positionShades = [100, 60, 40, 25];

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

/** 每个键在所在行里的份额：与 HarmonyOS 触屏键盘的 `layoutWeight` 一样，各行按份额铺满整行，键距之外的宽度按份额分给各键。 */
function keyFlex(weight: number): CSSProperties {
  return { flex: `${weight} 1 0%` };
}

/**
 * 用户所用手机键盘最近七天的按键热力图：每个键按它的次数相对于所画按键中最忙的那个着色，并标出它占全部按键次数的比例；几个键帽共用一个 id 时（9 键两侧的标点）只着色、不标占比。按过但这张键盘上没有的键列在下面的「其他键」里，键盘下方给出按得最多的拼写键。
 */
function KeyHeatmap({ counts, card }: { counts: Record<string, number>; card: string }) {
  const [chosen, setChosen] = useState<PhoneHeatmapLayout | null>(null);
  const othersId = useId();
  const layout: PhoneHeatmapLayout = chosen ?? (prefersNineKey(counts) ? "nine" : "full");
  const total = Object.values(counts).reduce((sum, count) => sum + count, 0);
  const peak = phoneHeatmapPeak(counts, layout);
  const leader = phoneHeatmapLeader(counts, layout);
  const shared = phoneHeatmapSharedCodes(layout);
  const others = phoneHeatmapOthers(counts, layout);
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
            <div className="flex gap-1" key={`${layout}-${rowIndex}`}>
              {row.map((key, keyIndex) => {
                const code = key.code;
                if (!code)
                  return (
                    <span
                      style={keyFlex(key.weight)}
                      aria-hidden="true"
                      key={`${rowIndex}-${keyIndex}`}
                    />
                  );
                const count = phoneHeatmapCount(counts, key);
                const level = summaryHeatLevel(count, peak);
                const delay = order++ * 18;
                return (
                  <span
                    className={`flex min-w-0 animate-ms-badge-in flex-col items-center justify-center gap-px overflow-hidden rounded-[6px] [animation-duration:0.4s] motion-reduce:animate-none ${layout === "nine" ? "h-[52px]" : "h-11"} ${level >= 4 ? "text-[color:var(--p-on-accent,#fff)]" : "text-[color:var(--p-text)]"}`}
                    style={{
                      ...keyFlex(key.weight),
                      background: accentMix(keyShades[level]),
                      animationDelay: `${delay}ms`,
                    }}
                    role="img"
                    aria-label={`${key.name ?? keyLabel(code)}，${formatZhNumber(count)} 次`}
                    key={`${rowIndex}-${keyIndex}`}
                  >
                    <span
                      className={`leading-none font-semibold whitespace-nowrap ${layout === "nine" ? "text-[15px]" : "text-[14px]"}`}
                      aria-hidden="true"
                    >
                      {key.label}
                    </span>
                    {total > 0 && !shared.has(code) && (
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
        {others.length > 0 && (
          <div className="flex flex-col gap-1.5 px-1">
            <p className={`m-0 text-[12px] ${subText}`} id={othersId}>
              其他键
            </p>
            <ul
              className="m-0 flex list-none flex-wrap gap-1.5 p-0 text-[12px]"
              aria-labelledby={othersId}
            >
              {others.map((key) => (
                <li
                  className="flex items-center gap-1.5 rounded-[6px] px-2 py-1"
                  style={{ background: accentMix(keyShades[0]) }}
                  aria-label={`${key.label}，${formatZhNumber(key.count)} 次`}
                  key={key.code}
                >
                  <i
                    className="block size-2.5 rounded-[2px]"
                    style={{ background: accentMix(keyShades[summaryHeatLevel(key.count, peak)]) }}
                    aria-hidden="true"
                  />
                  <span aria-hidden="true">{key.label}</span>
                  <span className={`tabular-nums ${subText}`} aria-hidden="true">
                    {percentTenths(key.count / total)}%
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className={`flex items-center justify-between gap-3 px-1 text-[12px] ${subText}`}>
          <span className="min-w-0 truncate">{headline}</span>
          <ShadeLegend shades={keyShades} />
        </div>
      </div>
    </Section>
  );
}

export function MobileStatisticsKeys({
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
                    className={`block h-2.5 flex-1 overflow-hidden rounded-[5px] ${statisticsTrack}`}
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
