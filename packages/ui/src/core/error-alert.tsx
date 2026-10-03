import type { ReactNode } from "react";

export interface ErrorAlertProps {
  children: ReactNode;
}

/** Shared error alert styling for account, community, and settings surfaces. */
export function ErrorAlert({ children }: ErrorAlertProps) {
  return (
    <p role="alert" className="error">
      {children}
    </p>
  );
}
