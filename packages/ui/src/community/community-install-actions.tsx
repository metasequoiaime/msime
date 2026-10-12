import type { ReactNode } from "react";
import { CommunityInstallButton } from "./community-install-button";
import { CommunityReplaceConfirmation } from "./community-replace-confirmation";

export interface CommunityInstallActionsProps {
  installed: boolean;
  installedContent: ReactNode;
  actionBusy: boolean;
  detailBusy: boolean;
  confirmReplace: boolean;
  replaceAriaLabel: string;
  replaceMessage: ReactNode;
  onInstall: (replace: boolean) => void;
  onCancelReplace: () => void;
}

/** Install action and replacement confirmation shared by package detail pages. */
export function CommunityInstallActions({
  installed,
  installedContent,
  actionBusy,
  detailBusy,
  confirmReplace,
  replaceAriaLabel,
  replaceMessage,
  onInstall,
  onCancelReplace,
}: CommunityInstallActionsProps) {
  return (
    <>
      {installed ? (
        installedContent
      ) : (
        <CommunityInstallButton
          actionBusy={actionBusy}
          detailBusy={detailBusy}
          confirmReplace={confirmReplace}
          onInstall={() => onInstall(false)}
        />
      )}
      {confirmReplace && (
        <CommunityReplaceConfirmation
          ariaLabel={replaceAriaLabel}
          message={replaceMessage}
          actionBusy={actionBusy}
          onConfirm={() => onInstall(true)}
          onCancel={onCancelReplace}
        />
      )}
    </>
  );
}
