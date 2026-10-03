import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsWarningProps {
  children: ReactNode;
  role?: "alert" | "status";
}

/** Shared status warning styling for settings validation messages. */
export function SettingsWarning({ children, role = "status" }: SettingsWarningProps) {
  return (
    <p className={settings.settingsWarning} role={role}>
      {children}
    </p>
  );
}
