import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityGalleryGridProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared responsive grid for community gallery cards. */
export function CommunityGalleryGrid({ children, className, ...props }: CommunityGalleryGridProps) {
  return (
    <div {...props} className={`${style.grid}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
