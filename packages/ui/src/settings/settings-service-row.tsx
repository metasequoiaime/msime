import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsServiceRowProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared service action row used by settings controls and status messages. */
export function SettingsServiceRow({ children, className, ...props }: SettingsServiceRowProps) {
  return (
    <div {...props} className={`${settings.serviceRow}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
