import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "../settings/settings-style";

export interface SkinPreviewSurfaceProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className" | "data-skin-preview"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared outer surface for candidate, skin, and toolbar previews. */
export function SkinPreviewSurface({ children, className, ...props }: SkinPreviewSurfaceProps) {
  return (
    <div
      {...props}
      className={`${settings.skinCardPreview}${className ? ` ${className}` : ""}`}
      data-skin-preview=""
    >
      {children}
    </div>
  );
}
