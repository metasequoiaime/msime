import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";

export interface ActionRowProps {
  title: ReactNode;
  description?: ReactNode;
  action?: () => void | Promise<void>;
  label: ReactNode;
  className?: string;
  disabled?: boolean;
  ariaLabel?: string;
  ariaBusy?: boolean;
}

/** A settings row with the shared secondary action button. */
export function ActionRow({
  title,
  description,
  action,
  label,
  className = "secondary",
  disabled = false,
  ariaLabel,
  ariaBusy,
}: ActionRowProps) {
  return (
    <Row title={title} description={description}>
      <ActionButton
        action={action}
        label={label}
        className={className}
        disabled={disabled}
        ariaLabel={ariaLabel}
        ariaBusy={ariaBusy}
      />
    </Row>
  );
}
