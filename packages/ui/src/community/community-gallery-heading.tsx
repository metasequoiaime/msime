import type { ReactNode } from "react";
import { InfoTip } from "../core/info-tip";
import * as style from "./community-style";

export interface CommunityGalleryHeadingProps {
  title: ReactNode;
  note: ReactNode;
  children?: ReactNode;
}

/** Shared heading layout used by community galleries and their action controls. */
export function CommunityGalleryHeading({ title, note, children }: CommunityGalleryHeadingProps) {
  return (
    <div className={style.heading}>
      <div className={style.headingBody}>
        {/* 图库是做什么的、装好后去哪里用收进标题旁的「?」，不在标题下常驻一行（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */}
        <div className="flex min-w-0 items-center gap-1">
          <h2 className={style.headingTitle}>{title}</h2>
          <InfoTip subject={title}>{note}</InfoTip>
        </div>
      </div>
      {children && <div className={style.headingActions}>{children}</div>}
    </div>
  );
}
