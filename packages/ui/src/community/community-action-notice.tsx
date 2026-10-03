import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityActionNoticeProps {
  children: ReactNode;
}

/** Shared status message styling for completed community actions. */
export function CommunityActionNotice({ children }: CommunityActionNoticeProps) {
  return (
    <p role="status" className={style.actionNotice}>
      {children}
    </p>
  );
}
