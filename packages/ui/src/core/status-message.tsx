import type { ComponentPropsWithoutRef, ReactNode } from "react";

export interface StatusMessageProps extends Omit<ComponentPropsWithoutRef<"p">, "children"> {
  children?: ReactNode;
}

/** Shared unstyled paragraph for live status messages. */
export function StatusMessage({ children, ...props }: StatusMessageProps) {
  return <p {...props}>{children}</p>;
}
