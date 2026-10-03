import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsPreviewBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "aria-label"
> {
  children?: ReactNode;
  className?: string;
  as?: "div" | "section";
  label?: ReactNode;
  "aria-label": string;
}

/** Shared preview surface used by settings pages and candidate appearance controls. */
export function SettingsPreviewBlock({
  children,
  className,
  as = "div",
  label = "预览",
  ...props
}: SettingsPreviewBlockProps) {
  const Element = as;
  return (
    <Element {...props} className={`${settings.groupPreview}${className ? ` ${className}` : ""}`}>
      <div className={settings.panelPreviewLabel}>{label}</div>
      {children}
    </Element>
  );
}
