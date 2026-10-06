import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityPageShellProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared surface shell for community galleries and detail views. */
export function CommunityPageShell({ children, className, ...props }: CommunityPageShellProps) {
  return (
    <div {...props} className={`${style.page}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
