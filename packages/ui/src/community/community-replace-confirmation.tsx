import type { ReactNode } from "react";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export interface CommunityReplaceConfirmationProps {
  ariaLabel: string;
  message: ReactNode;
  actionBusy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Shared confirmation for replacing an installed community item. */
export function CommunityReplaceConfirmation({
  ariaLabel,
  message,
  actionBusy,
  onConfirm,
  onCancel,
}: CommunityReplaceConfirmationProps) {
  return (
    <div className={style.confirmation} role="alertdialog" aria-label={ariaLabel}>
      <p>{message}</p>
      <div className={style.confirmationActions}>
        <ActionButton
          action={onConfirm}
          ariaBusy={actionBusy}
          className="danger"
          disabled={actionBusy}
          label="替换安装"
        />
        <ActionButton action={onCancel} ariaBusy={actionBusy} disabled={actionBusy} label="取消" />
      </div>
    </div>
  );
}
