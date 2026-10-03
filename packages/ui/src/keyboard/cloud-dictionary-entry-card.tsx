import * as cloud from "./cloud-panel-style";

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
    <article className={cloud.dictionaryItem}>
      <button
        type="button"
        className={cloud.dictionaryItemMain}
        aria-label={editLabel}
        onClick={onEdit}
        disabled={busy}
      >
        <strong>{entry.word}</strong>
        <small>
          {entry.code} · 权重 {entry.weight}
        </small>
      </button>
      <div className={cloud.dictionaryItemActions}>
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
      </div>
    </article>
  );
}
