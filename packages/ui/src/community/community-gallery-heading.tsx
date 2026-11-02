import type { ReactNode } from "react";
import { InfoTip } from "../core/info-tip";
import * as style from "./community-style";

export interface CommunityGalleryHeadingProps {
  title: ReactNode;
  /** 标题下的一句副标题（「发现创作者的配色与巧思」这类），常驻。 */
  note?: ReactNode;
  /** 这一栏怎么用、装好之后去哪里启用之类的说明：收进标题旁的「?」，不常驻页面（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  help?: ReactNode;
  children?: ReactNode;
}

/** Shared heading layout used by community galleries and their action controls. */
export function CommunityGalleryHeading({
  title,
  note,
  help,
  children,
}: CommunityGalleryHeadingProps) {
  return (
    <div className={style.heading}>
      <div className={style.headingBody}>
        <div className={style.headingTitleLine}>
          <h2 className={style.headingTitle}>{title}</h2>
          {help && <InfoTip subject={title}>{help}</InfoTip>}
        </div>
        {note && <p className={style.headingNote}>{note}</p>}
      </div>
      {children && <div className={style.headingActions}>{children}</div>}
    </div>
  );
}
