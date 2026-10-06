import type { ReactNode } from "react";
import { SettingSectionTitle } from "./setting-section-title";

export interface SettingSectionHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  as?: "div" | "label";
}

/** Shared title and optional description layout for standalone settings sections. */
export function SettingSectionHeader({
  title,
  description,
  children,
  as = "div",
}: SettingSectionHeaderProps) {
  const Header = as;
  return (
    <Header className="section-header">
      <SettingSectionTitle title={title} description={description} />
      {children}
    </Header>
  );
}

export interface SettingActionHeaderProps extends SettingSectionHeaderProps {
  children: ReactNode;
}

/** Shared title, description and action layout for standalone settings sections. */
export function SettingActionHeader(props: SettingActionHeaderProps) {
  return <SettingSectionHeader {...props} />;
}
