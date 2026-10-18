import { ActionButton } from "./action-button";

export interface DictionaryPaginationProps {
  busy: boolean;
  offset: number;
  hasMore: boolean;
  status: string;
  pageSize: number;
  onPageChange: (offset: number) => void;
}

/** 词库结果的分页控件，与新增按钮共用底栏；窄窗口内允许换行。 */
export function DictionaryPagination({
  busy,
  offset,
  hasMore,
  status,
  pageSize,
  onPageChange,
}: DictionaryPaginationProps) {
  return (
    <div className="flex max-w-full flex-wrap items-center justify-end gap-2 text-xs text-secondary">
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
