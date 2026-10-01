// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  useDictionaryManager,
  type DictionaryManagerClient,
} from "../../../../packages/ui/src/settings/use-dictionary-manager";

test("ignores a second all-dictionaries export while the first is pending", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const list = vi.fn(async (_offset: number, _limit: number, kind?: string) => {
    await pending;
    return {
      entries:
        kind === "pinyin"
          ? [{ kind: "pinyin" as const, key: "nihao", value: "你好", weight: 10000 }]
          : [],
      has_more: false,
    };
  });
  const saveExport = vi.fn().mockResolvedValue("/synthetic/export.txt");
  const client: DictionaryManagerClient = {
    dictionary: { list, edit: vi.fn().mockResolvedValue(undefined) },
    saveExport,
  };
  const { result } = renderHook(() =>
    useDictionaryManager({ client, confirm: vi.fn().mockResolvedValue(true) }),
  );

  let first!: Promise<void>;
  act(() => {
    first = result.current.exportAllPhrases();
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));

  let second!: Promise<void>;
  act(() => {
    second = result.current.exportAllPhrases();
  });
  expect(list).toHaveBeenCalledTimes(1);

  resolve();
  await act(async () => {
    await first;
    await second;
  });
  expect(saveExport).toHaveBeenCalledOnce();
  expect(result.current.phraseBusy).toBe(false);
});

test("a phrase list response from a replaced dictionary client is ignored", async () => {
  let resolve!: (value: {
    entries: Array<{ kind: "quick_phrase"; key: string; value: string; weight: number }>;
    has_more: boolean;
  }) => void;
  const pending = new Promise<{
    entries: Array<{ kind: "quick_phrase"; key: string; value: string; weight: number }>;
    has_more: boolean;
  }>((accept) => {
    resolve = accept;
  });
  const oldDictionary = {
    list: vi.fn().mockReturnValue(pending),
    edit: vi.fn().mockResolvedValue(undefined),
  };
  const nextDictionary = {
    list: vi.fn(),
    edit: vi.fn().mockResolvedValue(undefined),
  };
  const oldClient: DictionaryManagerClient = { dictionary: oldDictionary };
  const nextClient: DictionaryManagerClient = { dictionary: nextDictionary };
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm: vi.fn() }),
    { initialProps: { client: oldClient } },
  );

  let pendingLoad!: Promise<void>;
  act(() => {
    pendingLoad = result.current.loadPhrases();
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));
  rerender({ client: nextClient });
  resolve({
    entries: [{ kind: "quick_phrase", key: "shortcut", value: "旧结果", weight: 1 }],
    has_more: false,
  });
  await act(async () => pendingLoad);

  expect(result.current.phrases).toEqual([]);
});
