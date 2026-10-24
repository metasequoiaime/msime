import { rowHeading as heading } from "../core/row-style";
import { formatZhNumber } from "../core/format-number";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { dayLabel, formatActiveTime } from "./typing-statistics-helpers";
import { DETAIL_DAYS, type DailyDetailRow } from "./typing-statistics-model";

const footerNote = "mt-3.5 mb-0 text-xs leading-relaxed [color:var(--p-sub)]";

/** 按字符种类区分汉字和字母，其余文字归入「其他」，不按文本语言命名列。 */
const detailColumns = [
  "日期",
  "字数",
  "汉字",
  "字母",
  "数字",
  "标点",
  "其他",
  "活跃",
  "速度",
] as const;

export function StatisticsDailyDetails({ rows }: { rows: DailyDetailRow[] }) {
  const count = (value: number) => formatZhNumber(value);
  return (
    <section className="section m-0" aria-labelledby="statistics-details-title">
      <h2 className={heading} id="statistics-details-title">
        按日明细 · 最近 {DETAIL_DAYS} 天
      </h2>
      {rows.length === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          暂无输入记录
        </SettingsEmptyMessage>
      ) : (
        <div className="mt-3 overflow-x-auto">
          <table
            className="w-full border-collapse text-xs tabular-nums [&_td]:border-t [&_td]:border-edge [&_td]:px-2 [&_td]:py-1.5 [&_td]:text-right [&_td]:whitespace-nowrap [&_td:first-child]:text-left [&_th]:px-2 [&_th]:pb-1.5 [&_th]:text-right [&_th]:font-medium [&_th]:whitespace-nowrap [&_th]:text-muted [&_th:first-child]:text-left"
            aria-labelledby="statistics-details-title"
          >
            <thead>
              <tr>
                {detailColumns.map((title) => (
                  <th scope="col" key={title}>
                    {title}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.key}>
                  <td>{dayLabel(row.key)}</td>
                  <td className="font-medium text-body">{count(row.total)}</td>
                  {[row.han, row.latin, row.number, row.punctuation, row.other].map(
                    (value, index) => (
                      <td className="text-secondary" key={index}>
                        {count(value)}
                      </td>
                    ),
                  )}
                  <td>{row.activeMs === null ? "—" : formatActiveTime(row.activeMs)}</td>
                  <td>{row.speed === null ? "—" : `${count(Math.round(row.speed))} 字/分钟`}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className={footerNote}>
        列出最近 {DETAIL_DAYS}{" "}
        个有记录的日期，新的在上；「其他」含其他文字、表情、符号与历史未分类。活跃与速度显示「—」的日期早于活跃时长的记录。
      </p>
    </section>
  );
}
