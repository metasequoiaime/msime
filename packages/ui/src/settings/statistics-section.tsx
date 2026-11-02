import type { ReactNode } from "react";
import { InfoTip } from "../core/info-tip";
import { rowHeading } from "../core/row-style";

export interface StatisticsSectionProps {
  id: string;
  title: ReactNode;
  /** 解释这一块统计怎么算、记了什么：收进标题旁的「?」，不常驻页面（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  note?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** 统计页共用的标题区块外壳，保留各图表自己的内容和数据逻辑。 */
export function StatisticsSection({
  id,
  title,
  note,
  children,
  className = "section m-0",
}: StatisticsSectionProps) {
  const heading = (
    <h2 className={rowHeading} id={id}>
      {title}
    </h2>
  );
  return (
    <section className={className} aria-labelledby={id}>
      {note ? (
        <div className="flex min-w-0 items-center gap-1">
          {heading}
          <InfoTip subject={title}>{note}</InfoTip>
        </div>
      ) : (
        heading
      )}
      {children}
    </section>
  );
}
