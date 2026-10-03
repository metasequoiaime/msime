import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsPreviewLabelProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "as"
> {
  children?: ReactNode;
  className?: string;
  as?: "div" | "p";
}

/** Shared label style for settings preview surfaces and preview notices. */
export function SettingsPreviewLabel({
  children,
  className,
  as = "div",
  ...props
}: SettingsPreviewLabelProps) {
  const Element = as;
  return (
    <Element
      {...props}
      className={`${settings.panelPreviewLabel}${className ? ` ${className}` : ""}`}
    >
      {children}
    </Element>
  );
}
