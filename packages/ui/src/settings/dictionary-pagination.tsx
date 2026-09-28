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
      <button
        type="button"
        className="secondary"
        disabled={busy || offset === 0}
        onClick={() => onPageChange(Math.max(0, offset - pageSize))}
      >
        上一页
      </button>
      <span aria-live="polite">{status}</span>
      <button
        type="button"
        className="secondary"
        disabled={busy || !hasMore}
        onClick={() => onPageChange(offset + pageSize)}
      >
        下一页
      </button>
    </div>
  );
}
