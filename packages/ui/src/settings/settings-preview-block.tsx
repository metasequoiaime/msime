import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";
import { InfoTip } from "../core/info-tip";
import { SettingsPreviewLabel } from "./settings-preview-label";

export interface SettingsPreviewBlockProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "aria-label"
> {
  children?: ReactNode;
  className?: string;
  as?: "div" | "section";
  label?: ReactNode;
  /** 预览怎么看（「固定样例，不代表实际输入候选」）：收进标签旁的「?」，不常驻在预览上方。 */
  help?: ReactNode;
  "aria-label": string;
}

/** Shared preview surface used by settings pages and candidate appearance controls. */
export function SettingsPreviewBlock({
  children,
  className,
  as = "div",
  label = "预览",
  help,
  ...props
}: SettingsPreviewBlockProps) {
  const Element = as;
  return (
    <Element {...props} className={`${settings.groupPreview}${className ? ` ${className}` : ""}`}>
      {help ? (
        <div className="flex items-center gap-1">
          <SettingsPreviewLabel>{label}</SettingsPreviewLabel>
          <InfoTip subject={label}>{help}</InfoTip>
        </div>
      ) : (
        <SettingsPreviewLabel>{label}</SettingsPreviewLabel>
      )}
      {children}
    </Element>
  );
}
