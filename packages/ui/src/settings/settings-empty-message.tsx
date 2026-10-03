import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsEmptyMessageProps extends Omit<
  ComponentPropsWithoutRef<"p">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared empty and loading message style used by settings list surfaces. */
export function SettingsEmptyMessage({ children, className, ...props }: SettingsEmptyMessageProps) {
  return (
    <p {...props} className={`${settings.clipboardEmpty}${className ? ` ${className}` : ""}`}>
      {children}
    </p>
  );
}
