import type { ReactNode } from "react";
import { rowHeading } from "../core/row-style";

export interface StatisticsSectionProps {
  id: string;
  title: ReactNode;
  children: ReactNode;
  className?: string;
}

/** 统计页共用的标题区块外壳，保留各图表自己的内容和数据逻辑。 */
export function StatisticsSection({
  id,
  title,
  children,
  className = "section m-0",
}: StatisticsSectionProps) {
  return (
    <section className={className} aria-labelledby={id}>
      <h2 className={rowHeading} id={id}>
        {title}
      </h2>
      {children}
    </section>
  );
}
