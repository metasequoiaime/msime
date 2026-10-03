import type { HTMLAttributes, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsManagerActionsProps extends Omit<
  HTMLAttributes<HTMLElement>,
  "children" | "className"
> {
  as?: "div" | "span";
  children?: ReactNode;
  className?: string;
}

/** Shared action-button layout used by settings managers. */
export function SettingsManagerActions({
  as = "div",
  children,
  className,
  ...props
}: SettingsManagerActionsProps) {
  const Element = as;
  return (
    <Element {...props} className={`${settings.managerActions}${className ? ` ${className}` : ""}`}>
      {children}
    </Element>
  );
}
