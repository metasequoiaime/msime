import type { ReactNode } from "react";

export interface ActionButtonProps {
  action?: () => void | Promise<void>;
  label: ReactNode;
  className?: string;
  disabled?: boolean;
  ariaLabel?: string;
  ariaBusy?: boolean;
  ariaPressed?: boolean;
  ariaChecked?: boolean;
  ariaExpanded?: boolean;
  role?: "button" | "menuitem" | "switch";
}

/** Shared button for an action that may be asynchronous or unavailable. */
export function ActionButton({
  action,
  label,
  className = "secondary",
  disabled = false,
  ariaLabel,
  ariaBusy,
  ariaPressed,
  ariaChecked,
  ariaExpanded,
  role,
}: ActionButtonProps) {
  return (
    <button
      type="button"
      role={role}
      className={className}
      disabled={disabled || !action}
      aria-label={ariaLabel}
      aria-busy={ariaBusy}
      aria-pressed={ariaPressed}
      aria-checked={ariaChecked}
      aria-expanded={ariaExpanded}
      onClick={() => void action?.()}
    >
      {label}
    </button>
  );
}
