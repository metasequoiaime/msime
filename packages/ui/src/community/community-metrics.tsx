import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityMetricsProps extends Omit<
  ComponentPropsWithoutRef<"p">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared secondary text style for community download, rating, and package metrics. */
export function CommunityMetrics({ children, className, ...props }: CommunityMetricsProps) {
  return (
    <p {...props} className={`${style.metrics}${className ? ` ${className}` : ""}`}>
      {children}
    </p>
  );
}
