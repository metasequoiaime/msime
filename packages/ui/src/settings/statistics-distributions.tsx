import { formatZhNumber, formatZhPercent } from "../core/format-number";
import { DONUT_OUTER, DONUT_THICKNESS, donutSegments } from "./typing-chart";
import type { SelectionCounts } from "./typing-statistics-model";
import { palette, type Slice } from "./statistics-distribution-model";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { StatisticsSection } from "./statistics-section";

const rankChart = "mt-4 mb-[18px] flex flex-col gap-[11px]";
// 名称列容纳最长标签；计数和占比固定列宽，避免各行按内容计算而错位。
const rankRow =
  "grid grid-cols-[minmax(88px,1fr)_minmax(80px,2fr)_72px_48px] items-center gap-[9px] text-xs [&>span]:overflow-hidden [&>span]:text-ellipsis [&>span]:whitespace-nowrap [&>strong]:text-right [&>strong]:tabular-nums [&>strong]:text-secondary [&>small]:text-right [&>small]:tabular-nums [&>small]:text-muted";
const rankTrack = "h-[11px] overflow-hidden rounded-full bg-subtle";
const legendRow =
  "grid animate-row-reveal grid-cols-[22px_minmax(0,1fr)_auto_58px] items-center gap-[9px] motion-reduce:animate-none max-phone:grid-cols-[22px_minmax(0,1fr)_auto_50px] max-phone:gap-[7px] [&>strong]:font-medium [&>strong]:tabular-nums [&>small]:m-0 [&>small]:text-right [&>small]:tabular-nums";
const legendDot =
  "grid size-[22px] place-items-center rounded-[7px] text-xs font-[650] leading-none";

/** 字符和语言模式适合看整体占比的环形图；输入方案太多，改用带标签的单色排行。 */
type DistributionVariant = "donut" | "rank";

function ShapeChart({
  title,
  slices,
  variant,
  total,
}: {
  title: string;
  slices: Slice[];
  variant: DistributionVariant;
  total: number;
}) {
  if (variant === "rank") {
    const ranked = slices.filter((slice) => slice.count > 0).sort((a, b) => b.count - a.count);
    const peak = Math.max(1, ...ranked.map((slice) => slice.count));
    return (
      <div className={rankChart} role="img" aria-label={`${title}排行`}>
        {ranked.map((slice, index) => (
          <div
            className={`${rankRow} animate-row-reveal motion-reduce:animate-none`}
            style={{ animationDelay: `${Math.min(index, 8) * 0.03}s` }}
            key={slice.id}
            aria-label={`${slice.title} ${slice.count} 字符，${formatZhPercent(slice.count, total)}`}
          >
            <span>{slice.title}</span>
            <div className={rankTrack}>
              <i
                className="block h-full origin-left animate-bar-reveal rounded-[inherit] bg-accent motion-reduce:animate-none"
                style={{ width: `${(slice.count / peak) * 100}%` }}
              />
            </div>
            <strong>{formatZhNumber(slice.count)}</strong>
            <small>{formatZhPercent(slice.count, total)}</small>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div
      className="relative mx-auto size-[190px] shrink-0"
      role="img"
      aria-label={`${title}环形图`}
    >
      {/* 用 SVG 画环：环宽约为外半径的四分之一，扇区之间留底色缝隙，原先用 conic-gradient 加遮罩只能画出一道细线。 */}
      <svg className="block size-full" viewBox="0 0 100 100" aria-hidden="true">
        {donutSegments(slices).map((segment, index) =>
          "ring" in segment ? (
            <circle
              key={index}
              cx="50"
              cy="50"
              r={DONUT_OUTER - DONUT_THICKNESS / 2}
              fill="none"
              stroke={segment.color}
              strokeWidth={DONUT_THICKNESS}
              data-donut-segment=""
            />
          ) : (
            <path key={index} d={segment.d} fill={segment.color} data-donut-segment="" />
          ),
        )}
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <strong className="text-[25px] leading-tight font-semibold tabular-nums text-body">
          {formatZhNumber(total)}
        </strong>
        <span className="text-[11px] text-muted">字符</span>
      </div>
    </div>
  );
}

export function Distribution({
  title,
  slices,
  footer,
  variant,
  wide = false,
}: {
  title: string;
  slices: Slice[];
  footer?: string;
  variant: DistributionVariant;
  /** 宽度足够时，把环形图放到图例旁边。 */
  wide?: boolean;
}) {
  const total = slices.reduce((value, slice) => value + slice.count, 0);
  const visible = slices.filter((slice) => slice.count > 0 || slice.id !== "unknown");
  return (
    <StatisticsSection
      id={`statistics-${title}`}
      title={title}
      help={footer}
      className="section m-0 @container"
    >
      {total === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          暂无输入记录
        </SettingsEmptyMessage>
      ) : (
        // 标题独占一行，图表区整体放在标题下方；宽版按容器宽度而不是视口宽度决定是否并排，窄窗口里环形图回到图例上方，不再把图例挤成一字一行。
        <div
          className={
            variant === "donut"
              ? `mt-4 grid gap-5 ${wide ? "@min-[520px]:grid-cols-[190px_minmax(0,1fr)] @min-[520px]:items-center @min-[520px]:gap-8" : ""}`
              : undefined
          }
        >
          <ShapeChart title={title} slices={slices} variant={variant} total={total} />
          {/* 排行每行已有名称、计数和占比，不再另画图例。 */}
          {variant === "donut" && (
            <div className="flex flex-col gap-[11px]">
              {visible.map((slice, index) => (
                <div
                  // 各行按索引依次出现，支持任意行数，代替样式表中固定的八条延时规则。
                  className={legendRow}
                  style={{ animationDelay: `${Math.min(index, 8) * 0.03}s` }}
                  key={slice.id}
                  aria-label={`${slice.title} ${slice.count} 字符，${formatZhPercent(slice.count, total)}`}
                >
                  <span
                    className={legendDot}
                    style={{ color: slice.color, backgroundColor: `${slice.color}1a` }}
                    aria-hidden="true"
                  >
                    {slice.symbol}
                  </span>
                  <span>{slice.title}</span>
                  <strong>{formatZhNumber(slice.count)}</strong>
                  <small>{formatZhPercent(slice.count, total)}</small>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </StatisticsSection>
  );
}

/** 首页的九个位置分别计数，其余位置合并。 */
const RANK_SLOTS = 9;

/**
 * 候选上屏位置用单色表示，始终按位置排列，以保留从首选向后递减的形状。
 * 柱色与底色的对比不足 3:1，所以每行同时用文字显示计数和占比。
 */
export function CandidateRanks({ selections }: { selections: SelectionCounts | undefined }) {
  const ranks = Array.from({ length: RANK_SLOTS }, (_, index) => selections?.ranks?.[index] ?? 0);
  const beyond = selections?.beyond ?? 0;
  const total = ranks.reduce((sum, count) => sum + count, 0) + beyond;
  const peak = Math.max(1, ...ranks, beyond);
  const rows = [
    ...ranks.map((count, index) => ({
      id: `rank-${index + 1}`,
      label: `第 ${index + 1} 条`,
      count,
    })),
    { id: "beyond", label: "第 10 条以后", count: beyond },
  ];
  return (
    <StatisticsSection
      id="statistics-candidate-ranks"
      title="候选命中位置"
      help="每次上屏记录选中的是第几条候选，只记位置，不记任何文字。首选命中率越高，说明排序越贴合你的输入。"
    >
      <p className="mt-3.5 mb-1 flex items-baseline gap-2">
        <strong className="text-[30px] leading-[1.1] tabular-nums" aria-label="首选命中率">
          {formatZhPercent(ranks[0], total)}
        </strong>
        <span className="text-[13px] text-secondary" aria-hidden="true">
          首选命中率
        </span>
        <small className="ml-auto text-xs text-muted">
          {total === 0 ? "暂无记录" : `共 ${formatZhNumber(total)} 次上屏`}
        </small>
      </p>
      {total === 0 ? (
        <SettingsEmptyMessage centered>
          暂无候选记录。用水杉键盘上屏几次后再回来查看。
        </SettingsEmptyMessage>
      ) : (
        <div className={rankChart} role="img" aria-label="候选命中位置分布">
          {rows.map((row) => (
            <div
              className={rankRow}
              key={row.id}
              aria-label={`${row.label}：${row.count} 次，${formatZhPercent(row.count, total)}`}
            >
              <span>{row.label}</span>
              <div className={rankTrack}>
                <i
                  className="block h-full rounded-[inherit]"
                  style={{ width: `${(row.count / peak) * 100}%`, backgroundColor: palette[0] }}
                />
              </div>
              <strong>{formatZhNumber(row.count)}</strong>
              <small>{formatZhPercent(row.count, total)}</small>
            </div>
          ))}
        </div>
      )}
    </StatisticsSection>
  );
}
