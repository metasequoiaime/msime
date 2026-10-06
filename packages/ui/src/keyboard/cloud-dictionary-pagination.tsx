import * as cloud from "./cloud-panel-style";
import { ActionButton } from "../core/action-button";

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
      <ActionButton
        action={onPrevious}
        className={cloud.dictionaryButton}
        disabled={busy || offset === 0}
        label="上一页"
      />
      <span>第 {Math.floor(offset / 100) + 1} 页</span>
      <ActionButton
        action={onNext}
        className={cloud.dictionaryButton}
        disabled={busy || !hasMore}
        label="下一页"
      />
    </div>
  );
}
