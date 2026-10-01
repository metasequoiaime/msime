import { communityRating } from "./community-helpers";
import * as style from "./community-style";

export interface CommunitySkinCardMetricsProps {
  downloads: number;
  ratingCount: number;
  ratingAverage: number;
}

/** Shared download and rating metrics shown on community skin cards. */
export function CommunitySkinCardMetrics({
  downloads,
  ratingCount,
  ratingAverage,
}: CommunitySkinCardMetricsProps) {
  return (
    <span className={style.cardMetrics}>
      <span>↓ {downloads.toLocaleString("zh-CN")}</span>
      <span>☆ {communityRating(ratingCount, ratingAverage)}</span>
    </span>
  );
}
