import type { ReactNode } from "react";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";
import { CommunityConfirmationActions } from "./community-confirmation-actions";

export interface CommunityConfirmationProps {
  ariaLabel: string;
  message: ReactNode;
  actionBusy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  confirmLabel: ReactNode;
  actionsClassName?: string;
}

/** Shared confirmation surface and guarded confirm/cancel actions for community items. */
export function CommunityConfirmation({
  ariaLabel,
  message,
  actionBusy,
  onConfirm,
  onCancel,
  confirmLabel,
  actionsClassName,
}: CommunityConfirmationProps) {
  return (
    <div className={style.confirmation} role="alertdialog" aria-label={ariaLabel}>
      <p>{message}</p>
      <CommunityConfirmationActions className={actionsClassName}>
        <ActionButton
          action={onConfirm}
          ariaBusy={actionBusy}
          className="danger"
          disabled={actionBusy}
          label={confirmLabel}
        />
        <ActionButton action={onCancel} ariaBusy={actionBusy} disabled={actionBusy} label="取消" />
      </CommunityConfirmationActions>
    </div>
  );
}
