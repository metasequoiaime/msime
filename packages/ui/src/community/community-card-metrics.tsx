import { communityRating } from "./community-helpers";
import * as style from "./community-style";
import { formatZhNumber } from "../core/format-number";

export interface CommunityCardMetricsProps {
  downloads: number;
  ratingCount: number;
  ratingAverage: number;
  /** HarmonyOS 手机的皮肤卡片，只以 `N 次使用` 显示使用次数；评分留在详情页。 */
  look?: "harmony";
}

/** 社区卡片共用的下载量和评分指标。 */
export function CommunityCardMetrics({
  downloads,
  ratingCount,
  ratingAverage,
  look,
}: CommunityCardMetricsProps) {
  if (look === "harmony") {
    return <span className={style.harmonySkinMeta}>{formatZhNumber(downloads)} 次使用</span>;
  }
  return (
    <span className={style.cardMetrics}>
      <span>↓ {formatZhNumber(downloads)}</span>
      <span>☆ {communityRating(ratingCount, ratingAverage)}</span>
    </span>
  );
}
