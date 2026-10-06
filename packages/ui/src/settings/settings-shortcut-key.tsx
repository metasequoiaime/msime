import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsShortcutKeyProps extends Omit<
  ComponentPropsWithoutRef<"kbd">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared keycap styling for shortcut descriptions in settings. */
export function SettingsShortcutKey({ children, className, ...props }: SettingsShortcutKeyProps) {
  return (
    <kbd {...props} className={`${settings.shortcutKey}${className ? ` ${className}` : ""}`}>
      {children}
    </kbd>
  );
}
