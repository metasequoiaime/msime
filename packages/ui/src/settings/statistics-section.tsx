import type { ReactNode } from "react";
import { InfoTip } from "../core/info-tip";
import { rowHeading } from "../core/row-style";

export interface StatisticsSectionProps {
  id: string;
  title: ReactNode;
  /** 这张图怎么读、记了什么不记什么：收进标题旁的「?」，不常驻在图下（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  help?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** 统计页共用的标题区块外壳，保留各图表自己的内容和数据逻辑。 */
export function StatisticsSection({
  id,
  title,
  help,
  children,
  className = "section m-0",
}: StatisticsSectionProps) {
  return (
    <section className={className} aria-labelledby={id}>
      {help ? (
        <div className="flex items-center gap-1.5">
          <h2 className={rowHeading} id={id}>
            {title}
          </h2>
          <InfoTip subject={title}>{help}</InfoTip>
        </div>
      ) : (
        <h2 className={rowHeading} id={id}>
          {title}
        </h2>
      )}
      {children}
    </section>
  );
}
