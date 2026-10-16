import { formatZhNumber } from "../core/format-number";
import { CommunityCard } from "./community-card";
import { CommunityCardAuthor } from "./community-card-author";
import { communityRating, resourceKindTitle } from "./community-helpers";
import type { CommunityResource } from "./community-resource-model";
import * as style from "./community-style";

export interface CommunityResourceCardProps {
  item: CommunityResource;
  open: () => void;
}

/** 社区词库与回复模板的桌面卡片，打开动作由调用方提供。 */
export function CommunityResourceCard({ item, open }: CommunityResourceCardProps) {
  return (
    <CommunityCard onClick={open} aria-label={`查看${resourceKindTitle(item.kind)} ${item.name}`}>
      <span className={style.resourceIcon} aria-hidden="true">
        {item.kind === "dictionary" ? "字" : "话"}
      </span>
      <strong className={style.cardTitle}>{item.name}</strong>
      <CommunityCardAuthor
        author={item.author}
        owned={item.owned}
        removed={item.moderation === "removed"}
      />
      <span className={style.resourceDescription}>
        {item.description || (item.kind === "dictionary" ? "共享词条" : "回复模板")}
      </span>
      <span className={style.cardMetrics}>
        ☆ {communityRating(item.rating_count, item.rating_average)} · {formatZhNumber(item.saves)}{" "}
        人收藏
      </span>
    </CommunityCard>
  );
}
