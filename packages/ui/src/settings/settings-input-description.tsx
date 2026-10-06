import type { ReactNode } from "react";

export interface SettingsInputDescriptionProps {
  children: ReactNode;
  role?: "status";
}

/** Shared explanatory paragraph for settings controls that use the input description style. */
export function SettingsInputDescription({ children, role }: SettingsInputDescriptionProps) {
  return (
    <p className="input-setting-description" role={role}>
      {children}
    </p>
  );
}
