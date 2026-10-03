import type { ReactNode } from "react";

export interface SettingsErrorMessageProps {
  children: ReactNode;
}

/** Shared alert styling for settings error messages. */
export function SettingsErrorMessage({ children }: SettingsErrorMessageProps) {
  return (
    <p role="alert" className="error">
      {children}
    </p>
  );
}
