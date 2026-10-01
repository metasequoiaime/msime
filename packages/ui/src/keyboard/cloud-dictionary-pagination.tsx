import * as cloud from "./cloud-panel-style";

export interface CloudDictionaryPaginationProps {
  offset: number;
  hasMore: boolean;
  busy?: boolean;
  onPrevious: () => void;
  onNext: () => void;
}

/** Shared page navigation for cloud dictionary lists. */
export function CloudDictionaryPagination({
  offset,
  hasMore,
  busy = false,
  onPrevious,
  onNext,
}: CloudDictionaryPaginationProps) {
  return (
    <div className={cloud.dictionaryPagination}>
      <button
        className={cloud.dictionaryButton}
        type="button"
        onClick={onPrevious}
        disabled={busy || offset === 0}
      >
        上一页
      </button>
      <span>第 {Math.floor(offset / 100) + 1} 页</span>
      <button
        className={cloud.dictionaryButton}
        type="button"
        onClick={onNext}
        disabled={busy || !hasMore}
      >
        下一页
      </button>
    </div>
  );
}
