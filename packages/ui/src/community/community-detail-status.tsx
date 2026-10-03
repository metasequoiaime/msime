import { communityRating } from "./community-helpers";
import { CommunityMetrics } from "./community-metrics";
import { CommunityActionNotice } from "./community-action-notice";
import { StatusMessage } from "../core/status-message";
import { formatZhNumber } from "../core/format-number";

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
      <CommunityMetrics>
        {formatZhNumber(downloads)} 人下载 · {communityRating(ratingCount, ratingAverage)} ·{" "}
        {formatZhNumber(ratingCount)} 人评分
      </CommunityMetrics>
      {myRating > 0 && <CommunityMetrics>我的评分：{myRating} 星</CommunityMetrics>}
      {detailBusy && <StatusMessage role="status">{loadingText}</StatusMessage>}
      {actionNotice && <CommunityActionNotice>{actionNotice}</CommunityActionNotice>}
    </>
  );
}
