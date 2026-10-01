import type { ReactNode } from "react";

export interface SettingActionHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
}

/** Shared title, description and action layout for standalone settings sections. */
export function SettingActionHeader({ title, description, children }: SettingActionHeaderProps) {
  return (
    <div className="section-header">
      <span className="section-title">
        {title}
        {description !== undefined && <small>{description}</small>}
      </span>
      {children}
    </div>
  );
}
