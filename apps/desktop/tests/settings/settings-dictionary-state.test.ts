// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { useSettingsDictionaryState } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/use-dictionary-manager", () => ({
  useDictionaryManager: () => ({
    dictionaryKind: "quick_phrase",
    setPhraseForm: vi.fn(),
    loadPhrases: vi.fn(),
    exportPhrases: vi.fn(),
    exportAllPhrases: vi.fn(),
    importPhrases: vi.fn(),
    retryDictionaryFailure: vi.fn(),
    dismissDictionaryFailure: vi.fn(),
    savePhrase: vi.fn(),
    removePhrase: vi.fn(),
    resetLearnedData: vi.fn(),
  }),
}));

test("returns dictionary manager state with panel callbacks", () => {
  const { result } = renderHook(() =>
    useSettingsDictionaryState({
      client: {} as never,
      confirm: vi.fn(),
    }),
  );

  expect(result.current.dictionaryPanelActions.onAdd).toBeTypeOf("function");
  expect(result.current.dictionaryPanelActions.onQuery).toBeTypeOf("function");
});
