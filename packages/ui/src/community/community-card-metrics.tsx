import { communityRating } from "./community-helpers";
import * as style from "./community-style";

export interface CommunityCardMetricsProps {
  downloads: number;
  ratingCount: number;
  ratingAverage: number;
}

/** 社区卡片共用的下载量和评分指标。 */
export function CommunityCardMetrics({
  downloads,
  ratingCount,
  ratingAverage,
}: CommunityCardMetricsProps) {
  return (
    <span className={style.cardMetrics}>
      <span>↓ {downloads.toLocaleString("zh-CN")}</span>
      <span>☆ {communityRating(ratingCount, ratingAverage)}</span>
    </span>
  );
}
