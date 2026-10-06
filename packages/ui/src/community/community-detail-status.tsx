import { CommunityMetrics } from "./community-metrics";
import { CommunityActionNotice } from "./community-action-notice";
import { StatusMessage } from "../core/status-message";
import { CommunityRatingMetrics } from "./community-rating-metrics";

export interface CommunityDetailStatusProps {
  downloads: number;
  ratingCount: number;
  ratingAverage: number;
  myRating: number;
  detailBusy: boolean;
  actionNotice: string;
  loadingText?: string;
}

export function CommunityDetailStatus({
  downloads,
  ratingCount,
  ratingAverage,
  myRating,
  detailBusy,
  actionNotice,
  loadingText = "正在读取详情…",
}: CommunityDetailStatusProps) {
  return (
    <>
      <CommunityRatingMetrics
        count={downloads}
        countLabel="下载"
        ratingCount={ratingCount}
        ratingAverage={ratingAverage}
      />
      {myRating > 0 && <CommunityMetrics>我的评分：{myRating} 星</CommunityMetrics>}
      {detailBusy && <StatusMessage role="status">{loadingText}</StatusMessage>}
      {actionNotice && <CommunityActionNotice>{actionNotice}</CommunityActionNotice>}
    </>
  );
}
