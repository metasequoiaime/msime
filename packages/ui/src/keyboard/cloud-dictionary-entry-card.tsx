import { CloudDictionaryItem } from "./cloud-dictionary-item";

export interface CloudDictionaryEntryCardEntry {
  code: string;
  word: string;
  weight: number;
}

export interface CloudDictionaryEntryCardProps {
  entry: CloudDictionaryEntryCardEntry;
  editLabel: string;
  busy?: boolean;
  onEdit: () => void;
  onRemove: () => void;
  onDownload?: () => void;
}

/** Shared editable dictionary entry card used by the cloud list and complete catalog panels. */
export function CloudDictionaryEntryCard({
  entry,
  editLabel,
  busy = false,
  onEdit,
  onRemove,
  onDownload,
}: CloudDictionaryEntryCardProps) {
  return (
    <CloudDictionaryItem
      ariaLabel={editLabel}
      busy={busy}
      onClick={onEdit}
      actions={
        <>
          {onDownload && (
            <button
              type="button"
              className="secondary"
              aria-label={`下载到本机 ${entry.word}`}
              onClick={onDownload}
              disabled={busy}
            >
              下载到本机
            </button>
          )}
          <button type="button" className="secondary" onClick={onEdit} disabled={busy}>
            编辑
          </button>
          <button type="button" className="secondary" onClick={onRemove} disabled={busy}>
            删除
          </button>
        </>
      }
    >
      <strong>{entry.word}</strong>
      <small>
        {entry.code} · 权重 {entry.weight}
      </small>
    </CloudDictionaryItem>
  );
}
