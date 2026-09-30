import * as cloud from "./cloud-panel-style";

export interface CloudDictionaryEntryFormValue {
  code: string;
  word: string;
  weight: number;
}

export interface CloudDictionaryEntryFormProps {
  value: CloudDictionaryEntryFormValue;
  busy?: boolean;
  onChange: (patch: Partial<CloudDictionaryEntryFormValue>) => void;
  onSave: () => void;
  onCancel: () => void;
}

/** Shared editor used by the cloud dictionary list and complete catalog panels. */
export function CloudDictionaryEntryForm({
  value,
  busy = false,
  onChange,
  onSave,
  onCancel,
}: CloudDictionaryEntryFormProps) {
  return (
    <div className={cloud.dictionaryForm}>
      <label className={cloud.dictionaryField}>
        编码
        <input
          className={cloud.dictionaryInput}
          disabled={busy}
          value={value.code}
          onChange={(event) => onChange({ code: event.target.value })}
        />
      </label>
      <label className={cloud.dictionaryWordField}>
        词条
        <input
          className={cloud.dictionaryInput}
          disabled={busy}
          value={value.word}
          onChange={(event) => onChange({ word: event.target.value })}
        />
      </label>
      <label className={cloud.dictionaryField}>
        权重
        <input
          className={cloud.dictionaryInput}
          disabled={busy}
          type="number"
          min="0"
          value={value.weight}
          onChange={(event) => onChange({ weight: Number(event.target.value) })}
        />
      </label>
      <button type="button" onClick={onSave} disabled={busy}>
        保存
      </button>
      <button type="button" className="secondary" onClick={onCancel} disabled={busy}>
        取消
      </button>
    </div>
  );
}
