import type { ReactNode } from "react";
import { SettingSectionTitle } from "./setting-section-title";

export interface SettingFieldProps {
  label: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** Shared labeled field row used by settings sections with a control on the right. */
export function SettingField({
  label,
  description,
  children,
  className = "section-header",
}: SettingFieldProps) {
  return (
    <label className={className}>
      <SettingSectionTitle title={label} description={description} />
      {children}
    </label>
  );
}
