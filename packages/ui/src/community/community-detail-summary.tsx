import type { ReactNode } from "react";
import { CommunityDetailHeader, type CommunityDetailHeaderProps } from "./community-detail-header";
import { CommunityDetailStatus, type CommunityDetailStatusProps } from "./community-detail-status";

export interface CommunityDetailSummaryProps
  extends CommunityDetailHeaderProps, CommunityDetailStatusProps {
  metrics?: ReactNode;
}

/** 统一社区详情页的标题、补充指标与状态顺序。 */
export function CommunityDetailSummary({ metrics, ...props }: CommunityDetailSummaryProps) {
  return (
    <>
      <CommunityDetailHeader {...props} />
      {metrics}
      <CommunityDetailStatus {...props} />
    </>
  );
}
