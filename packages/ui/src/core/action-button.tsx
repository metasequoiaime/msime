import type { ReactNode } from "react";

export interface ActionButtonProps {
  action?: () => void | Promise<void>;
  label: ReactNode;
  className?: string;
  disabled?: boolean;
  ariaLabel?: string;
  ariaBusy?: boolean;
  ariaPressed?: boolean;
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
}: ActionButtonProps) {
  return (
    <button
      type="button"
      className={className}
      disabled={disabled || !action}
      aria-label={ariaLabel}
      aria-busy={ariaBusy}
      aria-pressed={ariaPressed}
      onClick={() => void action?.()}
    >
      {label}
    </button>
  );
}
