import { CommunityLoadMoreButton } from "./community-gallery-controls";
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
  if (!hasMore) return null;
  return (
    <>
      <CommunityLoadMoreButton disabled={busy} onClick={onLoadMore} />
      {busy && (
        <p role="status" className={style.notice}>
          {loadingText}
        </p>
      )}
    </>
  );
}
