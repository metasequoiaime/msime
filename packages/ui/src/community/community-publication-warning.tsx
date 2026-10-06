import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityPublicationWarningProps {
  children: ReactNode;
}

/** Shared disclosure warning styling used by community publication dialogs. */
export function CommunityPublicationWarning({ children }: CommunityPublicationWarningProps) {
  return <p className={style.warning}>{children}</p>;
}
