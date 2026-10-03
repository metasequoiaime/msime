import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityCardProps extends Omit<
  ComponentPropsWithoutRef<"button">,
  "children" | "className" | "type"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared button shell for cards in the community galleries. */
export function CommunityCard({ children, className, ...props }: CommunityCardProps) {
  return (
    <button {...props} type="button" className={`${style.card}${className ? ` ${className}` : ""}`}>
      {children}
    </button>
  );
}
