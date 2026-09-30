import { communityRating } from "./community-helpers";
import * as style from "./community-style";

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
      <p className={style.metrics}>
        {downloads.toLocaleString("zh-CN")} 人下载 · {communityRating(ratingCount, ratingAverage)} ·{" "}
        {ratingCount.toLocaleString("zh-CN")} 人评分
      </p>
      {myRating > 0 && <p className={style.metrics}>我的评分：{myRating} 星</p>}
      {detailBusy && <p role="status">{loadingText}</p>}
      {actionNotice && (
        <p role="status" className={style.actionNotice}>
          {actionNotice}
        </p>
      )}
    </>
  );
}
