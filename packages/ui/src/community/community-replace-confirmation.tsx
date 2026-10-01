import type { ReactNode } from "react";
import * as style from "./community-style";

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
        <button type="button" className="danger" disabled={actionBusy} onClick={onConfirm}>
          替换安装
        </button>
        <button type="button" className="secondary" disabled={actionBusy} onClick={onCancel}>
          取消
        </button>
      </div>
    </div>
  );
}
