import { useEffect } from "react";
import { useLatestRef } from "../core/use-latest-ref";

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
  const selectedIdRef = useLatestRef(selectedId);
  const onCloseRef = useLatestRef(onClose);

  useEffect(() => {
    if (!mobile || typeof window === "undefined") return;
    const onPopState = (event: PopStateEvent) => {
      const currentId = selectedIdRef.current;
      const detail = event.state?.communityDetail;
      if (currentId && !(detail?.kind === kind && detail.id === currentId)) {
        onCloseRef.current();
      }
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [kind, mobile, onCloseRef, selectedIdRef]);
}
