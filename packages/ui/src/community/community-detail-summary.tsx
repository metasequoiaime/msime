import type { ReactNode } from "react";
import { CommunityDetailHeader, type CommunityDetailHeaderProps } from "./community-detail-header";
import { CommunityDetailStatus, type CommunityDetailStatusProps } from "./community-detail-status";

export interface CommunityDetailSummaryProps
  extends Omit<CommunityDetailHeaderProps, "note">, CommunityDetailStatusProps {
  noteParts: readonly (string | null | undefined | false)[];
  metrics?: ReactNode;
}

/** 统一社区详情页的标题、补充指标与状态顺序。 */
export function CommunityDetailSummary({
  noteParts,
  metrics,
  ...props
}: CommunityDetailSummaryProps) {
  return (
    <>
      <CommunityDetailHeader {...props} note={noteParts.filter(Boolean).join(" · ")} />
      {metrics}
      <CommunityDetailStatus {...props} />
    </>
  );
}
