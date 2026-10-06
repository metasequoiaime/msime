import { ActionButton } from "./action-button";

export interface DictionaryPaginationProps {
  busy: boolean;
  offset: number;
  hasMore: boolean;
  status: string;
  pageSize: number;
  onPageChange: (offset: number) => void;
}

/** Previous/next controls for paged dictionary results. */
export function DictionaryPagination({
  busy,
  offset,
  hasMore,
  status,
  pageSize,
  onPageChange,
}: DictionaryPaginationProps) {
  return (
    <div className="flex items-center justify-center gap-4 text-xs text-secondary">
      <ActionButton
        action={() => onPageChange(Math.max(0, offset - pageSize))}
        disabled={busy || offset === 0}
        label="上一页"
      />
      <span aria-live="polite">{status}</span>
      <ActionButton
        action={() => onPageChange(offset + pageSize)}
        disabled={busy || !hasMore}
        label="下一页"
      />
    </div>
  );
}
