import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsRowStackProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared stacked layout for related rows inside a settings group. */
export function SettingsRowStack({ children, className, ...props }: SettingsRowStackProps) {
  return (
    <div {...props} className={`${settings.rowStack}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
