import type { ReactNode } from "react";

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
      <span className="section-title">
        {label}
        {description !== undefined && <small>{description}</small>}
      </span>
      {children}
    </label>
  );
}
