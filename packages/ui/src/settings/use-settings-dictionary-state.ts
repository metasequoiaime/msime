import type { SettingsClient } from "../index";
import { createDictionaryPanelActions } from "./dictionary-panel-actions";
import { useDictionaryManager } from "./use-dictionary-manager";

export interface UseSettingsDictionaryStateOptions {
  client: SettingsClient;
  confirm: Parameters<typeof useDictionaryManager>[0]["confirm"];
}

/** Combines dictionary manager state with the settings panel callback adapter. */
export function useSettingsDictionaryState({ client, confirm }: UseSettingsDictionaryStateOptions) {
  const dictionary = useDictionaryManager({ client, confirm });
  const dictionaryPanelActions = createDictionaryPanelActions({
    dictionaryKind: dictionary.dictionaryKind,
    setPhraseForm: dictionary.setPhraseForm,
    loadPhrases: dictionary.loadPhrases,
    exportPhrases: dictionary.exportPhrases,
    exportAllPhrases: dictionary.exportAllPhrases,
    importPhrases: dictionary.importPhrases,
    retryDictionaryFailure: dictionary.retryDictionaryFailure,
    dismissDictionaryFailure: dictionary.dismissDictionaryFailure,
    savePhrase: dictionary.savePhrase,
    removePhrase: dictionary.removePhrase,
    resetLearnedData: dictionary.resetLearnedData,
  });
  return { ...dictionary, dictionaryPanelActions } as const;
}
