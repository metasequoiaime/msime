import type { HTMLAttributes, ReactNode } from "react";
import { ActionButton } from "../core/action-button";

export interface NativePanelHeaderProps {
  title?: ReactNode;
  onClose: () => void;
  closeDisabled?: boolean;
  className?: string;
  drag?: HTMLAttributes<HTMLElement>;
  children?: ReactNode;
}

/** Shared title bar and close control for native keyboard panels. */
export function NativePanelHeader({
  title,
  onClose,
  closeDisabled = false,
  className = "native-panel-header",
  drag,
  children,
}: NativePanelHeaderProps) {
  return (
    <header className={className} {...drag}>
      {title !== undefined && <span>{title}</span>}
      {children}
      <ActionButton action={onClose} ariaLabel="关闭" disabled={closeDisabled} label="×" />
    </header>
  );
}
