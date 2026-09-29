import type { RefObject } from "react";
import type { DictionaryEntry, LocalDictionaryKind } from "../dictionary/dictionary-file";
import { dictionaryKindKeyHint } from "../dictionary/dictionary-messages";
import { localDictionaryKinds } from "../dictionary/dictionary-kinds";
import * as settings from "./settings-style";

export interface DictionaryPhraseForm {
  key: string;
  value: string;
  weight: number;
  previous: DictionaryEntry | null;
}

export interface DictionaryEntriesProps {
  kind: LocalDictionaryKind;
  entries: DictionaryEntry[];
  form: DictionaryPhraseForm | null;
  busy: boolean;
  listRef: RefObject<HTMLUListElement | null>;
  onFormChange: (form: DictionaryPhraseForm) => void;
  onSave: () => void;
  onCancel: () => void;
  onEdit: (entry: DictionaryEntry) => void;
  onRemove: (entry: DictionaryEntry) => void;
}

/** Editor and result list for one page of local dictionary entries. */
export function DictionaryEntries({
  kind,
  entries,
  form,
  busy,
  listRef,
  onFormChange,
  onSave,
  onCancel,
  onEdit,
  onRemove,
}: DictionaryEntriesProps) {
  return (
    <>
      {form && (
        <div className={settings.phraseForm}>
          <label>
            编码{" "}
            <input
              value={form.key}
              readOnly={form.previous?.source === "bundled"}
              onChange={(event) => onFormChange({ ...form, key: event.target.value })}
            />
            <small className={settings.keyHint}>{dictionaryKindKeyHint(kind)}</small>
          </label>
          <label>
            {kind === "quick_phrase" ? "短语" : "词条"}{" "}
            <input
              value={form.value}
              readOnly={form.previous?.source === "bundled"}
              onChange={(event) => onFormChange({ ...form, value: event.target.value })}
            />
          </label>
          <label>
            权重{" "}
            <input
              type="number"
              value={form.weight}
              onChange={(event) => onFormChange({ ...form, weight: Number(event.target.value) })}
            />
          </label>
          <button type="button" disabled={busy} onClick={onSave}>
            保存
          </button>
          <button type="button" className="secondary" disabled={busy} onClick={onCancel}>
            取消
          </button>
        </div>
      )}
      {entries.length === 0 ? (
        <p className={settings.empty}>
          点击查询后查看
          {localDictionaryKinds.find(([value]) => value === kind)?.[1] ?? "词库"}词条
        </p>
      ) : (
        <ul ref={listRef} className={settings.phraseList} aria-label="词库查询结果">
          {entries.map((entry, index) => (
            <li key={`${entry.key}-${entry.value}-${index}`}>
              <span>
                <code>{entry.key}</code>　{entry.value}　<small>{entry.weight}</small>
                {entry.source === "bundled" && (
                  <>
                    {" "}
                    <small className={settings.bundledBadge}>内置</small>
                  </>
                )}
              </span>
              <span>
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => onEdit(entry)}
                >
                  {entry.source === "bundled" ? "调权重" : "编辑"}
                </button>{" "}
                <button
                  type="button"
                  className="secondary"
                  disabled={busy}
                  onClick={() => onRemove(entry)}
                >
                  删除
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
