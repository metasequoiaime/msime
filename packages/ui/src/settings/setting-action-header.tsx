import type { ReactNode } from "react";

export interface SettingSectionHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
}

/** Shared title and optional description layout for standalone settings sections. */
export function SettingSectionHeader({ title, description, children }: SettingSectionHeaderProps) {
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

export interface SettingActionHeaderProps extends SettingSectionHeaderProps {
  children: ReactNode;
}

/** Shared title, description and action layout for standalone settings sections. */
export function SettingActionHeader(props: SettingActionHeaderProps) {
  return <SettingSectionHeader {...props} />;
}
