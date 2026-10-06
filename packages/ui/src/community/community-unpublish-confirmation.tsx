import type { ReactNode } from "react";
import { CommunityConfirmation } from "./community-confirmation";

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
    <CommunityConfirmation
      ariaLabel={ariaLabel}
      message={message}
      actionBusy={actionBusy}
      onConfirm={onConfirm}
      onCancel={onCancel}
      confirmLabel={confirmLabel}
      actionsClassName={actionsClassName}
    />
  );
}
