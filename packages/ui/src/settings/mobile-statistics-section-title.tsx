import type { ReactNode } from "react";
import { subText } from "./mobile-statistics-style";

export interface MobileStatisticsSectionTitleProps {
  title: string;
  trailing?: ReactNode;
}

/** 移动端统计卡片共用的小标题行，末端可带一个操作控件。 */
export function MobileStatisticsSectionTitle({
  title,
  trailing,
}: MobileStatisticsSectionTitleProps) {
  return (
    <div className="flex min-h-5 items-center justify-between gap-3 px-1">
      <h3 className={`m-0 text-[13px] font-normal ${subText}`}>{title}</h3>
      {trailing}
    </div>
  );
}
