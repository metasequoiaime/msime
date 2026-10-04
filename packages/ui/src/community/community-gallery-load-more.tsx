import { CommunityLoadMoreButton } from "./community-gallery-controls";
import { StatusMessage } from "../core/status-message";
import * as style from "./community-style";

export interface CommunityGalleryLoadMoreProps {
  hasMore: boolean;
  busy: boolean;
  loadingText: string;
  onLoadMore: () => void;
}

/** Shared load-more action and status used by community gallery pages. */
export function CommunityGalleryLoadMore({
  hasMore,
  busy,
  loadingText,
  onLoadMore,
}: CommunityGalleryLoadMoreProps) {
  if (!hasMore && !busy) return null;
  return (
    <>
      {hasMore && <CommunityLoadMoreButton disabled={busy} onClick={onLoadMore} />}
      {busy && (
        <StatusMessage role="status" className={style.notice}>
          {loadingText}
        </StatusMessage>
      )}
    </>
  );
}
