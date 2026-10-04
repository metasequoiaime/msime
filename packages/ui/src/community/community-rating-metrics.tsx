import { formatZhNumber } from "../core/format-number";
import { communityRating } from "./community-helpers";
import { CommunityMetrics } from "./community-metrics";

export interface CommunityRatingMetricsProps {
  count: number;
  /** The kind of count shown before the rating, such as 下载 or 收藏. */
  countLabel: string;
  ratingCount: number;
  ratingAverage: number;
}

/** Shared count and rating line used by community detail pages. */
export function CommunityRatingMetrics({
  count,
  countLabel,
  ratingCount,
  ratingAverage,
}: CommunityRatingMetricsProps) {
  return (
    <CommunityMetrics>
      {formatZhNumber(count)} 人{countLabel} · {communityRating(ratingCount, ratingAverage)} ·{" "}
      {formatZhNumber(ratingCount)} 人评分
    </CommunityMetrics>
  );
}
