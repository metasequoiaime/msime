import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "../settings/settings-style";

export interface SkinPreviewStageProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "data-skin-stage"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared stage wrapper for each candidate, keyboard, or toolbar skin preview. */
export function SkinPreviewStage({ children, className, ...props }: SkinPreviewStageProps) {
  return (
    <div
      {...props}
      className={`${settings.skinPreviewStage}${className ? ` ${className}` : ""}`}
      data-skin-stage=""
    >
      {children}
    </div>
  );
}
