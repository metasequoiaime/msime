import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityConfirmationActionsProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared flex layout for actions inside community confirmations. */
export function CommunityConfirmationActions({
  children,
  className,
  ...props
}: CommunityConfirmationActionsProps) {
  return (
    <div {...props} className={`${style.confirmationActions}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
