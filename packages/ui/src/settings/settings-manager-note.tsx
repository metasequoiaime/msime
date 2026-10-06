import type { HTMLAttributes, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsManagerNoteProps {
  children: ReactNode;
  className?: string;
  role?: HTMLAttributes<HTMLParagraphElement>["role"];
}

/** Shared explanatory paragraph used in settings manager blocks. */
export function SettingsManagerNote({ children, className, role }: SettingsManagerNoteProps) {
  return (
    <p className={`${settings.managerNote}${className ? ` ${className}` : ""}`} role={role}>
      {children}
    </p>
  );
}
