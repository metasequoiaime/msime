import type { ReactNode } from "react";
import { CommunityRemovedBadge, type CommunityModeration } from "./community-report";
import * as style from "./community-style";

export interface CommunityDetailHeaderProps {
  title: string;
  note: ReactNode;
  owned: boolean;
  moderation?: CommunityModeration | null;
  ownedLabel?: ReactNode;
  description?: ReactNode;
}

/** Shared title, author metadata, ownership badges, and description for community detail pages. */
export function CommunityDetailHeader({
  title,
  note,
  owned,
  moderation,
  ownedLabel = "我的作品",
  description,
}: CommunityDetailHeaderProps) {
  return (
    <>
      <div className={style.detailTitle}>
        <div className={style.headingBody}>
          <h2 className={style.headingTitle}>{title}</h2>
          <p className={style.headingNote}>{note}</p>
        </div>
        {owned && <span className={style.detailBadge}>{ownedLabel}</span>}
        <CommunityRemovedBadge owned={owned} moderation={moderation} />
      </div>
      {description && <p className={style.description}>{description}</p>}
    </>
  );
}
