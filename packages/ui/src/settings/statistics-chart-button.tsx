import type { ReactNode } from "react";

/** 统计图表共用的受控选择按钮，外观和内容由调用方提供。 */
export function StatisticsChartButton({
  className,
  title,
  ariaLabel,
  selected,
  onClick,
  children,
}: {
  className: string;
  title: string;
  ariaLabel: string;
  selected: boolean;
  onClick: () => void;
  children?: ReactNode;
}) {
  return (
    <button
      type="button"
      className={className}
      title={title}
      aria-label={ariaLabel}
      aria-pressed={selected}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
