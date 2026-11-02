import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { InfoTip } from "../core/info-tip";
import * as settings from "./settings-style";
import { SettingsPreviewLabel } from "./settings-preview-label";

export interface SettingsPreviewBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "aria-label"
> {
  children?: ReactNode;
  className?: string;
  as?: "div" | "section";
  label?: ReactNode;
  /** 预览画的是什么、和实际输入有什么不同：收进「预览」旁的「?」。 */
  note?: ReactNode;
  "aria-label": string;
}

/** Shared preview surface used by settings pages and candidate appearance controls. */
export function SettingsPreviewBlock({
  children,
  className,
  as = "div",
  label = "预览",
  note,
  ...props
}: SettingsPreviewBlockProps) {
  const Element = as;
  return (
    <Element {...props} className={`${settings.groupPreview}${className ? ` ${className}` : ""}`}>
      {note ? (
        <div className="flex items-center gap-1">
          <SettingsPreviewLabel>{label}</SettingsPreviewLabel>
          <InfoTip subject={label}>{note}</InfoTip>
        </div>
      ) : (
        <SettingsPreviewLabel>{label}</SettingsPreviewLabel>
      )}
      {children}
    </Element>
  );
}
