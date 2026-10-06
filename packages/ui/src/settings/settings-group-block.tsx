import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsGroupBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared padded container for content that sits inside a settings group. */
export function SettingsGroupBlock({ children, className, ...props }: SettingsGroupBlockProps) {
  return (
    <div {...props} className={`${settings.groupBlock}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
