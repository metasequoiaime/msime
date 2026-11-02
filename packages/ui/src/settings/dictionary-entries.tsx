import type { RefObject } from "react";
import type {
  DictionaryEntry,
  DictionaryLookupEntry,
  DictionaryLookupKind,
  LocalDictionaryKind,
} from "../dictionary/dictionary-file";
import { dictionaryKindKeyHint } from "../dictionary/dictionary-messages";
import {
  isResourceDictionaryKind,
  localDictionaryKinds,
  resourceDictionaryKinds,
} from "../dictionary/dictionary-kinds";
import * as settings from "./settings-style";
import { SettingsPhraseForm } from "./settings-phrase-form";
import { InfoTip } from "../core/info-tip";
import { ActionButton } from "./action-button";
import { QUICK_PHRASE_DICTIONARY_CURSOR_NOTE } from "./plugin-editor-model";
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
  emptyMessage?: string;
  listRef: RefObject<HTMLUListElement | null>;
  onFormChange: (form: DictionaryPhraseForm) => void;
  onSave: () => void;
  onCancel: () => void;
  onEdit: (entry: DictionaryEntry) => void;
  onRemove: (entry: DictionaryEntry) => void;
}

/** 本地词条的单页编辑与查询结果。 */
export function DictionaryEntries({
  kind,
  entries,
  form,
  busy,
  emptyMessage,
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
            {/* 光标标记怎么写是解释：在 label 里只能放悬停的「?」，文本框照旧以 label 命名。 */}
            {kind === "quick_phrase" && (
              <InfoTip passive subject="短语">
                {QUICK_PHRASE_DICTIONARY_CURSOR_NOTE}
              </InfoTip>
            )}
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
      <DictionaryEntryList
        kind={kind}
        entries={entries}
        busy={busy}
        emptyMessage={emptyMessage}
        listRef={listRef}
        onEdit={onEdit}
        onRemove={onRemove}
      />
    </>
  );
}

/** 查询结果共用同一列表；资源类型即使来源标记异常也不出现写入操作。 */
export function DictionaryEntryList({
  kind,
  entries,
  busy,
  emptyMessage,
  listRef,
  onEdit,
  onRemove,
}: {
  kind: DictionaryLookupKind;
  entries: DictionaryLookupEntry[];
  busy: boolean;
  emptyMessage?: string;
  listRef: RefObject<HTMLUListElement | null>;
  onEdit?: (entry: DictionaryEntry) => void;
  onRemove?: (entry: DictionaryEntry) => void;
}) {
  const resource = isResourceDictionaryKind(kind);
  const columns = resource ? settings.phraseResourceColumns : settings.phrasePersonalColumns;
  const reading = kind === "japanese" || kind === "korean_hanja";

  return entries.length === 0 ? (
    <SettingsEmptyMessage compact>
      {emptyMessage || (
        <>
          输入关键词后查看
          {[...localDictionaryKinds, ...resourceDictionaryKinds].find(
            ([value]) => value === kind,
          )?.[1] ?? "词库"}
          词条
        </>
      )}
    </SettingsEmptyMessage>
  ) : (
    <div className={settings.phraseTable}>
      <div className={`${settings.phraseListHeader} ${columns}`} aria-hidden="true">
        <span>{reading ? "读音" : "编码"}</span>
        <span>{kind === "quick_phrase" ? "短语" : "词条"}</span>
        {!resource && (
          <>
            <span className="text-right">权重</span>
            <span>来源</span>
            <span className="text-right">操作</span>
          </>
        )}
      </div>
      <ul ref={listRef} className={settings.phraseList} aria-label="词库查询结果">
        {entries.map((entry, index) => (
          <li
            key={`${entry.key}-${entry.value}-${index}`}
            className={`${settings.phraseListItem} ${columns}`}
          >
            <code className={settings.phraseEntryKey} title={entry.key}>
              {entry.key}
            </code>
            <span className={settings.phraseEntryValue} title={entry.value}>
              {entry.value}
            </span>
            {!resource && (
              <>
                <small className="text-right text-muted" title="权重">
                  {entry.weight}
                </small>
                <small className="text-xs text-muted">
                  {entry.source === "bundled" ? "内置" : "个人"}
                </small>
                <span className={settings.phraseEntryActions}>
                  {!isResourceDictionaryKind(entry.kind) &&
                    entry.source !== "bundled" &&
                    onEdit &&
                    onRemove && (
                      <>
                        <ActionButton
                          action={() => onEdit(entry as DictionaryEntry)}
                          disabled={busy}
                          label="编辑"
                        />
                        <ActionButton
                          action={() => onRemove(entry as DictionaryEntry)}
                          disabled={busy}
                          label="删除"
                        />
                      </>
                    )}
                </span>
              </>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
