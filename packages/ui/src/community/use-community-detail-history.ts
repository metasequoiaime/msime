import { useMobilePopState } from "../settings/use-mobile-pop-state";

export interface CommunityDetailHistoryOptions {
  mobile: boolean;
  kind: string;
  selectedId: string | null;
  onClose: () => void;
}

/** Closes a community detail when mobile history leaves that detail route. */
export function useCommunityDetailHistory({
  mobile,
  kind,
  selectedId,
  onClose,
}: CommunityDetailHistoryOptions): void {
  useMobilePopState(mobile, (event) => {
    const detail = event.state?.communityDetail;
    if (selectedId && !(detail?.kind === kind && detail.id === selectedId)) onClose();
  });
}
