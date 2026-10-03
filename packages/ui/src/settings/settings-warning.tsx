import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsWarningProps {
  children: ReactNode;
}

/** Shared status warning styling for settings validation messages. */
export function SettingsWarning({ children }: SettingsWarningProps) {
  return (
    <p className={settings.settingsWarning} role="status">
      {children}
    </p>
  );
}
