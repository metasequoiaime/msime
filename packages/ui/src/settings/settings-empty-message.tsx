import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsEmptyMessageProps extends Omit<
  ComponentPropsWithoutRef<"p">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
  compact?: boolean;
}

/** Shared empty and loading message style used by settings list surfaces. */
export function SettingsEmptyMessage({
  children,
  className,
  compact = false,
  ...props
}: SettingsEmptyMessageProps) {
  return (
    <p
      {...props}
      className={`${compact ? settings.empty : settings.clipboardEmpty}${className ? ` ${className}` : ""}`}
    >
      {children}
    </p>
  );
}
