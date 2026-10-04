import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsEmptyMessageProps extends Omit<
  ComponentPropsWithoutRef<"p">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
  compact?: boolean;
  centered?: boolean;
}

/** Shared empty and loading message style used by settings list surfaces. */
export function SettingsEmptyMessage({
  children,
  className,
  compact = false,
  centered = false,
  ...props
}: SettingsEmptyMessageProps) {
  return (
    <p
      {...props}
      className={`${centered ? "mt-0.5 mb-3.5 text-center text-muted" : compact ? settings.empty : settings.clipboardEmpty}${className ? ` ${className}` : ""}`}
    >
      {children}
    </p>
  );
}
