import type { ReactNode } from "react";

/** Compact explanatory text used beneath statistics charts and heatmaps. */
export function StatisticsNote({ children }: { children: ReactNode }) {
  return <p className="mt-[7px] mb-0 text-xs text-muted">{children}</p>;
}
