import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsManagerBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared flex container for settings manager content and results. */
export function SettingsManagerBlock({ children, className, ...props }: SettingsManagerBlockProps) {
  return (
    <div {...props} className={`${settings.managerBlock}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
