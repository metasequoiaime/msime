import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsGroupNoteProps {
  children: ReactNode;
  className?: string;
  role?: "status";
}

/** Shared explanatory paragraph used inside settings groups. */
export function SettingsGroupNote({ children, className, role }: SettingsGroupNoteProps) {
  return (
    <p className={`${settings.groupNote}${className ? ` ${className}` : ""}`} role={role}>
      {children}
    </p>
  );
}
