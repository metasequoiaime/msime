import type { ComponentPropsWithoutRef, ReactNode } from "react";

export interface ErrorAlertProps extends Omit<ComponentPropsWithoutRef<"p">, "children" | "role"> {
  children: ReactNode;
}

/** Shared error alert styling for account, community, and settings surfaces. */
export function ErrorAlert({ children, className = "error", ...props }: ErrorAlertProps) {
  return (
    <p {...props} role="alert" className={className}>
      {children}
    </p>
  );
}
