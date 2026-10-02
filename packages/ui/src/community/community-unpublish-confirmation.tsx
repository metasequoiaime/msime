import type { ReactNode } from "react";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export interface CommunityUnpublishConfirmationProps {
  ariaLabel: string;
  message: ReactNode;
  actionBusy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  actionsClassName?: string;
  confirmLabel?: string;
}

/** Shared confirmation dialog for removing a published community item. */
export function CommunityUnpublishConfirmation({
  ariaLabel,
  message,
  actionBusy,
  onConfirm,
  onCancel,
  actionsClassName,
  confirmLabel = "确认下架",
}: CommunityUnpublishConfirmationProps) {
  return (
    <div className={style.confirmation} role="alertdialog" aria-label={ariaLabel}>
      <p>{message}</p>
      <div className={actionsClassName}>
        <ActionButton
          action={onConfirm}
          ariaBusy={actionBusy}
          className="danger"
          disabled={actionBusy}
          label={confirmLabel}
        />
        <ActionButton action={onCancel} ariaBusy={actionBusy} disabled={actionBusy} label="取消" />
      </div>
    </div>
  );
}
