import { expect, test, vi } from "vitest";
import { createDictionaryPanelActions } from "@msime/ui";

test("creates dictionary panel callbacks with the current kind and defaults", async () => {
  const setPhraseForm = vi.fn();
  const loadPhrases = vi.fn().mockResolvedValue(undefined);
  const exportPhrases = vi.fn().mockResolvedValue(undefined);
  const exportAllPhrases = vi.fn().mockResolvedValue(undefined);
  const importPhrases = vi.fn().mockResolvedValue(undefined);
  const retryDictionaryFailure = vi.fn().mockResolvedValue(undefined);
  const dismissDictionaryFailure = vi.fn().mockResolvedValue(undefined);
  const savePhrase = vi.fn().mockResolvedValue(undefined);
  const removePhrase = vi.fn().mockResolvedValue(undefined);
  const resetLearnedData = vi.fn().mockResolvedValue(undefined);
  const actions = createDictionaryPanelActions({
    dictionaryKind: "quick_phrase",
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
  });
  const file = {} as File;
  const entry = { key: "ni", value: "你", weight: 10, kind: "quick_phrase" } as never;

  actions.onQuery();
  actions.onAdd();
  actions.onExportCurrent();
  actions.onExportAll();
  actions.onImport(file);
  actions.onRetry("request-1");
  actions.onDismiss("request-1");
  actions.onSavePhrase();
  actions.onRemovePhrase(entry);
  actions.onLoadPhrases("pinyin", 12);
  actions.onResetLearnedData();

  expect(loadPhrases).toHaveBeenNthCalledWith(1, "quick_phrase", 0);
  expect(loadPhrases).toHaveBeenNthCalledWith(2, "pinyin", 12);
  expect(setPhraseForm).toHaveBeenCalledWith({ key: "", value: "", weight: 10, previous: null });
  expect(importPhrases).toHaveBeenCalledWith(file);
  expect(retryDictionaryFailure).toHaveBeenCalledWith("request-1");
  expect(dismissDictionaryFailure).toHaveBeenCalledWith("request-1");
  expect(removePhrase).toHaveBeenCalledWith(entry);
  await Promise.resolve();
  expect(resetLearnedData).toHaveBeenCalledOnce();
});
