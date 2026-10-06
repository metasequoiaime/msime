import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityFieldProps {
  label: ReactNode;
  children: ReactNode;
}

/** Shared styled label wrapper used by community form fields. */
export function CommunityField({ label, children }: CommunityFieldProps) {
  return (
    <label className={style.field}>
      {label}
      {children}
    </label>
  );
}
