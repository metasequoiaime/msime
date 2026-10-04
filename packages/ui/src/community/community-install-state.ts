import { useState } from "react";

export interface CommunityInstallState {
  installed: boolean;
  setInstalled: (installed: boolean) => void;
  confirmReplace: boolean;
  setConfirmReplace: (confirm: boolean) => void;
  closeDetail: () => void;
}

/** Shares install-result state and detail cleanup across community package galleries. */
export function useCommunityInstallState(
  actionBusy: boolean,
  closeGalleryDetail: () => void,
): CommunityInstallState {
  const [installed, setInstalled] = useState(false);
  const [confirmReplace, setConfirmReplace] = useState(false);

  const closeDetail = () => {
    if (actionBusy) return;
    closeGalleryDetail();
    setInstalled(false);
    setConfirmReplace(false);
  };

  return { installed, setInstalled, confirmReplace, setConfirmReplace, closeDetail };
}
