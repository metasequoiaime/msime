import type { DictionaryEntry, LocalDictionaryKind } from "../dictionary/dictionary-file";
import type { DictionaryPhraseForm } from "./dictionary-entries";

export interface CreateDictionaryPanelActionsOptions {
  dictionaryKind: LocalDictionaryKind;
  setPhraseForm: (form: DictionaryPhraseForm | null) => void;
  loadPhrases: (kind?: LocalDictionaryKind, offset?: number) => Promise<void>;
  exportPhrases: () => Promise<void>;
  exportAllPhrases: () => Promise<void>;
  importPhrases: (file: File) => Promise<void>;
  retryDictionaryFailure: (requestId: string) => Promise<void>;
  dismissDictionaryFailure: (requestId: string) => Promise<void>;
  savePhrase: () => Promise<void>;
  removePhrase: (entry: DictionaryEntry) => Promise<void>;
  resetLearnedData: () => Promise<void>;
}

/** Adapts dictionary-manager operations to the settings panel callback surface. */
export function createDictionaryPanelActions({
  dictionaryKind,
  setPhraseForm,
  loadPhrases,
  exportPhrases,
  exportAllPhrases,
  importPhrases,
  retryDictionaryFailure,
  dismissDictionaryFailure,
  savePhrase,
  removePhrase,
  resetLearnedData,
}: CreateDictionaryPanelActionsOptions) {
  return {
    onQuery: () => void loadPhrases(dictionaryKind, 0),
    onAdd: () => setPhraseForm({ key: "", value: "", weight: 10, previous: null }),
    onExportCurrent: () => void exportPhrases(),
    onExportAll: () => void exportAllPhrases(),
    onImport: (file: File) => void importPhrases(file),
    onRetry: (requestId: string) => void retryDictionaryFailure(requestId),
    onDismiss: (requestId: string) => void dismissDictionaryFailure(requestId),
    onSavePhrase: () => void savePhrase(),
    onRemovePhrase: (entry: DictionaryEntry) => void removePhrase(entry),
    onLoadPhrases: (kind?: LocalDictionaryKind, offset?: number) => void loadPhrases(kind, offset),
    onResetLearnedData: () => void resetLearnedData(),
  } as const;
}
