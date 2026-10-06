import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityNoticeProps {
  children: ReactNode;
}

/** Shared centered notice styling for community empty states and inline guidance. */
export function CommunityNotice({ children }: CommunityNoticeProps) {
  return <p className={style.notice}>{children}</p>;
}
