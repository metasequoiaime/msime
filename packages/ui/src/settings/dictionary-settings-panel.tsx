import type { RefObject } from "react";
import {
  DICTIONARY_PAGE_SIZE,
  type DictionaryEntry,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "../dictionary/dictionary-file";
import type { DictionaryClient, DictionaryFailure, DictionaryManifest } from "../index";
import { DictionaryFailuresNotice } from "./dictionary-failures-notice";
import { DictionaryManagerControls } from "./dictionary-manager-controls";
import { DictionaryManagerHeader } from "./dictionary-manager-header";
import { DictionaryPagination } from "./dictionary-pagination";
import { DictionaryEntries, type DictionaryPhraseForm } from "./dictionary-entries";
import { DictionaryManifestCard } from "./dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "./personal-dictionary-import-card";
import { LearningDataSection } from "./learning-data-section";
import * as settings from "./settings-style";

export interface DictionarySettingsPanelProps {
  disabled: boolean;
  hidden: boolean;
  dictionary?: DictionaryClient;
  dictionaryManifest?: () => Promise<DictionaryManifest>;
  platform?: string;
  macos: boolean;
  resetLearnedData?: () => Promise<void>;
  phraseBusy: boolean;
  dictionaryFormat: LocalDictionaryFormat;
  setDictionaryFormat: (format: LocalDictionaryFormat) => void;
  onQuery: () => void;
  onAdd: () => void;
  onExportCurrent: () => void;
  onExportAll: () => void;
  onImport: (file: File) => void;
  dictionaryPendingCount: number;
  dictionaryFailures: DictionaryFailure[];
  dictionarySnapshotError: string;
  canRetry: boolean;
  canDismiss: boolean;
  onRetry: (requestId: string) => void;
  onDismiss: (requestId: string) => void;
  dictionaryKind: LocalDictionaryKind;
  phraseSearch: string;
  setDictionaryKind: (kind: LocalDictionaryKind) => void;
  setPhraseSearch: (search: string) => void;
  phraseError: string;
  phraseNotice: string;
  phrases: DictionaryEntry[];
  setPhrases: (entries: DictionaryEntry[]) => void;
  phraseForm: DictionaryPhraseForm | null;
  phraseListRef: RefObject<HTMLUListElement | null>;
  setPhraseForm: (form: DictionaryPhraseForm | null) => void;
  onSavePhrase: () => void;
  onRemovePhrase: (entry: DictionaryEntry) => void;
  onLoadPhrases: (kind?: LocalDictionaryKind, offset?: number) => void;
  onTurnPage: (offset: number) => void;
  phrasePage: { offset: number; hasMore: boolean; status: string };
  onResetLearnedData: () => void;
}

/** Complete local dictionary management page, including packaged and personal dictionaries. */
export function DictionarySettingsPanel({
  disabled,
  hidden,
  dictionary,
  dictionaryManifest,
  platform,
  macos,
  resetLearnedData,
  phraseBusy,
  dictionaryFormat,
  setDictionaryFormat,
  onQuery,
  onAdd,
  onExportCurrent,
  onExportAll,
  onImport,
  dictionaryPendingCount,
  dictionaryFailures,
  dictionarySnapshotError,
  canRetry,
  canDismiss,
  onRetry,
  onDismiss,
  dictionaryKind,
  phraseSearch,
  setDictionaryKind,
  setPhraseSearch,
  phraseError,
  phraseNotice,
  phrases,
  setPhrases,
  phraseForm,
  phraseListRef,
  setPhraseForm,
  onSavePhrase,
  onRemovePhrase,
  onLoadPhrases,
  onTurnPage,
  phrasePage,
  onResetLearnedData,
}: DictionarySettingsPanelProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="词库">
      {dictionaryManifest && <DictionaryManifestCard read={dictionaryManifest} />}
      {dictionary?.importPersonal && (
        <PersonalDictionaryImportCard dictionary={dictionary} platform={platform} />
      )}
      {dictionary && (
        <>
          <DictionaryManagerHeader
            disabled={phraseBusy}
            dictionaryFormat={dictionaryFormat}
            onQuery={onQuery}
            onAdd={onAdd}
            onExportCurrent={onExportCurrent}
            onExportAll={onExportAll}
            onImport={onImport}
            onFormatChange={setDictionaryFormat}
          />
          {dictionaryPendingCount > 0 && (
            <p className="input-setting-description" role="status">
              {dictionaryPendingCount} 项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。
            </p>
          )}
          {dictionarySnapshotError && (
            <p role="alert" className="error">
              {dictionarySnapshotError}
            </p>
          )}
          <DictionaryFailuresNotice
            failures={dictionaryFailures}
            busy={phraseBusy}
            canRetry={canRetry}
            canDismiss={canDismiss}
            onRetry={onRetry}
            onDismiss={onDismiss}
          />
          <DictionaryManagerControls
            kind={dictionaryKind}
            format={dictionaryFormat}
            search={phraseSearch}
            disabled={phraseBusy}
            onKindChange={(kind) => {
              setDictionaryKind(kind);
              if (kind !== "pinyin" && dictionaryFormat === "hans") setDictionaryFormat("standard");
              setPhrases([]);
              void onLoadPhrases(kind);
            }}
            onFormatChange={setDictionaryFormat}
            onSearchChange={setPhraseSearch}
          />
          {phraseError && (
            <p role="alert" className="error">
              {phraseError}
            </p>
          )}
          {phraseNotice && (
            <p role="status" className={settings.empty}>
              {phraseNotice}
            </p>
          )}
          <DictionaryEntries
            kind={dictionaryKind}
            entries={phrases}
            form={phraseForm}
            busy={phraseBusy}
            listRef={phraseListRef}
            onFormChange={setPhraseForm}
            onSave={onSavePhrase}
            onCancel={() => setPhraseForm(null)}
            onEdit={(entry) =>
              setPhraseForm({
                key: entry.key,
                value: entry.value,
                weight: entry.weight,
                previous: entry,
              })
            }
            onRemove={onRemovePhrase}
          />
          <DictionaryPagination
            busy={phraseBusy}
            offset={phrasePage.offset}
            hasMore={phrasePage.hasMore}
            status={phrasePage.status}
            pageSize={DICTIONARY_PAGE_SIZE}
            onPageChange={onTurnPage}
          />
        </>
      )}
      {macos && resetLearnedData && (
        <LearningDataSection disabled={phraseBusy} onReset={onResetLearnedData} />
      )}
    </fieldset>
  );
}
