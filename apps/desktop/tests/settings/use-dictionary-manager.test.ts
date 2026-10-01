// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  useDictionaryManager,
  type DictionaryManagerClient,
} from "../../../../packages/ui/src/settings/use-dictionary-manager";

test("ignores a second phrase save while the first is pending", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const edit = vi.fn().mockReturnValue(pending);
  const list = vi.fn().mockResolvedValue({ entries: [], has_more: false });
  const client: DictionaryManagerClient = {
    dictionary: { list, edit },
  };
  const { result } = renderHook(() =>
    useDictionaryManager({ client, confirm: vi.fn().mockResolvedValue(true) }),
  );
  act(() =>
    result.current.setPhraseForm({
      key: "synthetic",
      value: "合成词条",
      weight: 1,
      previous: null,
    }),
  );

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.savePhrase();
    second = result.current.savePhrase();
  });
  expect(edit).toHaveBeenCalledOnce();

  resolve();
  await act(async () => {
    await first;
    await second;
  });
  expect(edit).toHaveBeenCalledOnce();
});

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

test("ignores a second dictionary failure retry while the first is pending", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const retry = vi.fn().mockReturnValue(pending);
  const list = vi.fn().mockResolvedValue({ entries: [], has_more: false });
  const client: DictionaryManagerClient = {
    dictionary: { list, edit: vi.fn().mockResolvedValue(undefined), retry },
  };
  const { result } = renderHook(() =>
    useDictionaryManager({ client, confirm: vi.fn().mockResolvedValue(true) }),
  );

  let first!: Promise<void>;
  act(() => {
    first = result.current.retryDictionaryFailure("request-1");
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));

  let second!: Promise<void>;
  act(() => {
    second = result.current.retryDictionaryFailure("request-1");
  });
  expect(retry).toHaveBeenCalledOnce();

  resolve();
  await act(async () => {
    await first;
    await second;
  });
  expect(list).toHaveBeenCalledOnce();
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

test("a learning reset response from a replaced dictionary client is ignored", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const oldClient: DictionaryManagerClient = {
    dictionary: { list: vi.fn(), edit: vi.fn().mockResolvedValue(undefined) },
    resetLearnedData: vi.fn().mockReturnValue(pending),
  };
  const nextClient: DictionaryManagerClient = {
    dictionary: { list: vi.fn(), edit: vi.fn().mockResolvedValue(undefined) },
    resetLearnedData: vi.fn(),
  };
  const confirm = vi.fn().mockResolvedValue(true);
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm }),
    { initialProps: { client: oldClient } },
  );
  const existing = { kind: "quick_phrase" as const, key: "shortcut", value: "保留", weight: 1 };
  act(() => result.current.setPhrases([existing]));

  let pendingReset!: Promise<void>;
  act(() => {
    pendingReset = result.current.resetLearnedData();
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));
  rerender({ client: nextClient });
  resolve();
  await act(async () => pendingReset);

  expect(result.current.phrases).toEqual([existing]);
});

test("a phrase save response from a replaced dictionary client is ignored", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const oldClient: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit: vi.fn().mockReturnValue(pending),
    },
  };
  const nextClient: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit: vi.fn().mockResolvedValue(undefined),
    },
  };
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm: vi.fn() }),
    { initialProps: { client: oldClient } },
  );
  act(() =>
    result.current.setPhraseForm({
      key: "shortcut",
      value: "新词条",
      weight: 1,
      previous: null,
    }),
  );

  let pendingSave!: Promise<void>;
  act(() => {
    pendingSave = result.current.savePhrase();
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));
  rerender({ client: nextClient });
  resolve();
  await act(async () => pendingSave);

  expect(result.current.phraseForm).toEqual({
    key: "shortcut",
    value: "新词条",
    weight: 1,
    previous: null,
  });
});

test("a phrase removal response from a replaced dictionary client is ignored", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const existing = { kind: "quick_phrase" as const, key: "shortcut", value: "保留", weight: 1 };
  const oldClient: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [existing], has_more: false }),
      edit: vi.fn().mockReturnValue(pending),
    },
  };
  const nextList = vi.fn().mockResolvedValue({ entries: [existing], has_more: false });
  const nextClient: DictionaryManagerClient = {
    dictionary: { list: nextList, edit: vi.fn().mockResolvedValue(undefined) },
  };
  const confirm = vi.fn().mockResolvedValue(true);
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm }),
    { initialProps: { client: oldClient } },
  );
  act(() => result.current.setPhrases([existing]));

  let pendingRemove!: Promise<void>;
  act(() => {
    pendingRemove = result.current.removePhrase(existing);
  });
  await waitFor(() => expect(oldClient.dictionary?.edit).toHaveBeenCalledOnce());
  rerender({ client: nextClient });
  resolve();
  await act(async () => pendingRemove);

  expect(oldClient.dictionary?.list).not.toHaveBeenCalled();
  expect(nextList).not.toHaveBeenCalled();
  expect(result.current.phrases).toEqual([existing]);
});
