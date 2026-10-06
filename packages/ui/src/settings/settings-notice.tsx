import type { ReactNode } from "react";

export interface SettingsNoticeProps {
  children: ReactNode;
  role?: "status";
}

/** Shared notice styling for settings messages. */
export function SettingsNotice({ children, role }: SettingsNoticeProps) {
  return (
    <p className="notice" role={role}>
      {children}
    </p>
  );
}
