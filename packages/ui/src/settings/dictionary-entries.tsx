import type { RefObject } from "react";
import type { DictionaryEntry, LocalDictionaryKind } from "../dictionary/dictionary-file";
import { dictionaryKindKeyHint } from "../dictionary/dictionary-messages";
import { localDictionaryKinds } from "../dictionary/dictionary-kinds";
import * as settings from "./settings-style";
import { SettingsPhraseForm } from "./settings-phrase-form";
import { ActionButton } from "./action-button";
import { SettingsEmptyMessage } from "./settings-empty-message";

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
        <SettingsPhraseForm>
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
          <ActionButton action={onSave} ariaBusy={busy} className="" disabled={busy} label="保存" />
          <ActionButton action={onCancel} disabled={busy} label="取消" />
        </SettingsPhraseForm>
      )}
      {entries.length === 0 ? (
        <SettingsEmptyMessage compact>
          点击查询后查看
          {localDictionaryKinds.find(([value]) => value === kind)?.[1] ?? "词库"}词条
        </SettingsEmptyMessage>
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
                <ActionButton
                  action={() => onEdit(entry)}
                  disabled={busy}
                  label={entry.source === "bundled" ? "调权重" : "编辑"}
                />{" "}
                <ActionButton action={() => onRemove(entry)} disabled={busy} label="删除" />
              </span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
