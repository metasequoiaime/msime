import type { ReactNode } from "react";
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
        <h2 className={style.headingTitle}>{title}</h2>
        <p className={style.headingNote}>{note}</p>
      </div>
      {children && <div className={style.headingActions}>{children}</div>}
    </div>
  );
}
