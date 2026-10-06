import type { ReactNode } from "react";
import { CommunityConfirmation } from "./community-confirmation";

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
    <CommunityConfirmation
      ariaLabel={ariaLabel}
      message={message}
      actionBusy={actionBusy}
      onConfirm={onConfirm}
      onCancel={onCancel}
      confirmLabel="替换安装"
    />
  );
}
