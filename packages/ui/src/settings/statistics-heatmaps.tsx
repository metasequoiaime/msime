import { useMemo } from "react";
import { formatZhNumber } from "../core/format-number";
import {
  keyboardHeatmapModel,
  keyHeatLevel,
  scopedKeyCounts,
  type DailyKeyCounts,
  type KeyboardHeatmapKey,
} from "./keyboard-heatmap";
import { statisticsHeatmapWeeks } from "./typing-statistics-helpers";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { StatisticsNote } from "./statistics-note";
import { StatisticsChartButton } from "./statistics-chart-button";
import { StatisticsSection } from "./statistics-section";

const footerNote = "mt-3.5 mb-0 text-xs leading-relaxed [color:var(--p-sub)]";
// 热力图按等级选择强调色透明度；零级使用空轨道底色。
const heatLevels = [
  "bg-subtle",
  "bg-accent opacity-[0.28]",
  "bg-accent opacity-[0.46]",
  "bg-accent opacity-[0.68]",
  "bg-accent",
] as const;
const heatCell = "block size-[14px] box-border rounded-[3px] border-0 p-0";
// 每周是一列七个固定高度的格子，星期标签使用同样的行轨道以保持对齐。
const heatWeek = "grid grid-rows-[repeat(7,14px)] gap-[3px]";

export function StatisticsHeatmap({
  days,
  selectedDay,
  onSelect,
}: {
  days: Record<string, number>;
  selectedDay: string | null;
  onSelect: (key: string) => void;
}) {
  const weeks = useMemo(() => statisticsHeatmapWeeks(days), [days]);
  const maximum = Math.max(1, ...weeks.flat().map((day) => day.count));
  // 月份只标在包含一号的周列上，与该月的起始位置对齐。
  const monthLabels = weeks.map((week) => {
    const first = week.find((day) => day.key.endsWith("-01"));
    return first ? `${first.label.split("月")[0]}月` : "";
  });
  return (
    <div className="mt-2" role="group" aria-label="每日输入热力图">
      <div className="overflow-x-auto pb-1 [scrollbar-width:thin]">
        <div
          className="flex h-4 w-max min-w-full items-end text-[9px] text-muted [&>span:first-child]:w-[18px] [&>span:first-child]:flex-[0_0_18px] [&>span:not(:first-child)]:mr-[3px] [&>span:not(:first-child)]:w-[14px] [&>span:not(:first-child)]:overflow-visible [&>span:not(:first-child)]:whitespace-nowrap"
          aria-hidden="true"
        >
          <span />
          {monthLabels.map((label, index) => (
            <span key={index}>{label}</span>
          ))}
        </div>
        <div className="flex w-max min-w-full gap-1">
          <div
            className={`${heatWeek} w-[18px] flex-[0_0_18px] text-right text-[9px] leading-[14px] text-muted`}
            aria-hidden="true"
          >
            {["一", "", "三", "", "五", "", ""].map((label, index) => (
              <span key={index}>{label}</span>
            ))}
          </div>
          <div className="flex gap-[3px]" role="grid">
            {weeks.map((week, index) => (
              <div className={heatWeek} key={index}>
                {week.map((day) => {
                  if (day.future)
                    return (
                      <span className={`${heatCell} invisible`} key={day.key} aria-hidden="true" />
                    );
                  const level =
                    day.count === 0 ? 0 : Math.max(1, Math.ceil((day.count / maximum) * 4));
                  return (
                    <StatisticsChartButton
                      className={`${heatCell} ${heatLevels[level]} cursor-pointer${selectedDay === day.key ? " outline-2 outline-offset-1 outline-[#e59b43]" : ""}`}
                      key={day.key}
                      title={`${day.label}：${day.count > 0 ? `${formatZhNumber(day.count)} 字符` : "无记录"}`}
                      ariaLabel={`热力图：${day.label}，${day.count} 字符`}
                      selected={selectedDay === day.key}
                      onClick={() => onSelect(day.key)}
                    />
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="mt-2 flex items-center gap-1 text-[10px] text-muted" aria-hidden="true">
        <span>少</span>
        {heatLevels.map((shade, level) => (
          <i className={`block size-3 rounded-[2px] ${shade}`} key={level} />
        ))}
        <span>多</span>
      </div>
    </div>
  );
}

function KeyboardHeatmapRows({
  rows,
  maximum,
  className,
}: {
  rows: KeyboardHeatmapKey[][];
  maximum: number;
  className: string;
}) {
  return (
    <div className={className}>
      {rows.map((row, rowIndex) => (
        <div className="flex gap-1" key={rowIndex}>
          {row.map((key, index) => {
            const style = { flexGrow: key.weight, flexBasis: 0 };
            // 没有键码的格子只负责对齐行缩进，不是按键。
            if (!key.code)
              return <span className="min-w-0" style={style} key={index} aria-hidden="true" />;
            const level = keyHeatLevel(key.count, maximum);
            const name = `${key.name}，${formatZhNumber(key.count)} 次`;
            return (
              <span
                role="img"
                className="relative grid h-9 min-w-0 place-items-center overflow-hidden rounded-[5px] border border-edge text-[11px] leading-none max-phone:h-8 max-phone:text-[10px]"
                style={style}
                key={index}
                title={name}
                aria-label={name}
              >
                <i className={`absolute inset-0 ${heatLevels[level]}`} aria-hidden="true" />
                <span
                  className={`relative overflow-hidden px-0.5 text-ellipsis whitespace-nowrap ${level >= 3 ? "[color:var(--p-on-accent,#fff)]" : "text-secondary"}`}
                  aria-hidden="true"
                >
                  {key.label}
                </span>
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
}

/** 按当前范围绘制按键次数：实体键盘使用 ANSI 布局，触屏使用 26 键布局，并在有记录时补充九键网格。 */
export function KeyboardHeatmap({
  dailyKeys,
  scopeKeys,
  scopeLabel,
  mobile,
  platform,
}: {
  dailyKeys: DailyKeyCounts | undefined;
  scopeKeys: string[] | null;
  scopeLabel: string;
  mobile: boolean;
  platform?: string;
}) {
  const model = keyboardHeatmapModel(scopedKeyCounts(dailyKeys, scopeKeys), mobile, platform);
  const count = (value: number) => formatZhNumber(value);
  return (
    <StatisticsSection id="statistics-keys-title" title={`按键热力图 · ${scopeLabel}`}>
      {model.total === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          这段时间还没有按键记录
        </SettingsEmptyMessage>
      ) : (
        <>
          <StatisticsNote>共 {count(model.total)} 次按键，颜色越深按得越多</StatisticsNote>
          <div role="group" aria-label="按键热力图">
            <KeyboardHeatmapRows
              rows={model.rows}
              maximum={model.maximum}
              className="mt-3 flex flex-col gap-1"
            />
            {model.nineRows && (
              <div role="group" aria-label="九宫格按键">
                <p className="mt-3.5 mb-0 text-xs text-muted">九宫格</p>
                <KeyboardHeatmapRows
                  rows={model.nineRows}
                  maximum={model.maximum}
                  className="mx-auto mt-2 flex max-w-[260px] flex-col gap-1"
                />
              </div>
            )}
            {model.others.length > 0 && (
              <div className="mt-3.5">
                <p className="m-0 text-xs text-muted" id="statistics-keys-others">
                  其他键
                </p>
                <ul
                  className="m-0 mt-2 flex list-none flex-wrap gap-1.5 p-0 text-xs"
                  aria-labelledby="statistics-keys-others"
                >
                  {model.others.map((key) => (
                    <li
                      className="flex items-center gap-1.5 rounded-[5px] border border-edge px-2 py-1"
                      key={key.code}
                      aria-label={`${key.label}，${count(key.count)} 次`}
                    >
                      <i
                        className={`block size-2.5 rounded-[2px] ${heatLevels[keyHeatLevel(key.count, model.maximum)]}`}
                        aria-hidden="true"
                      />
                      <span aria-hidden="true">{key.label}</span>
                      <strong
                        className="font-medium tabular-nums text-secondary"
                        aria-hidden="true"
                      >
                        {count(key.count)}
                      </strong>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
          <div className="mt-2 flex items-center gap-1 text-[10px] text-muted" aria-hidden="true">
            <span>少</span>
            {heatLevels.map((shade, level) => (
              <i className={`block size-3 rounded-[2px] ${shade}`} key={level} />
            ))}
            <span>多</span>
          </div>
          <ol
            className="m-0 mt-3.5 flex list-none flex-col gap-1.5 p-0 text-xs"
            aria-label="最常按的键"
          >
            {model.top.map((key, index) => (
              <li className="flex items-center gap-2" key={key.code}>
                <span className="w-4 text-right tabular-nums text-muted">{index + 1}</span>
                <span className="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                  {key.label}
                </span>
                <strong className="font-medium tabular-nums text-secondary">
                  {count(key.count)} 次
                </strong>
              </li>
            ))}
          </ol>
        </>
      )}
      <p className={footerNote}>
        只保存每个键每天被按下的次数，不保存按键顺序和输入内容；按住不放只算一次，密码框中的按键不计入。
      </p>
    </StatisticsSection>
  );
}
