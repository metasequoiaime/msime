// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useDictionaryManager,
  type DictionaryManagerClient,
} from "../../../../packages/ui/src/settings/use-dictionary-manager";

afterEach(cleanup);

test("Chinese word queries retain matching host rows and report an empty result", async () => {
  const entry = { kind: "pinyin" as const, key: "he'cheng", value: "合成", weight: 10 };
  const list = vi
    .fn()
    .mockResolvedValueOnce({ entries: [entry], has_more: false })
    .mockResolvedValueOnce({ entries: [], has_more: false });
  const client: DictionaryManagerClient = { dictionary: { list, edit: vi.fn() } };
  const { result } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  expect(result.current.phrasePage.status).toBe("");
  act(() => result.current.setPhraseSearch("合成"));
  await act(async () => {
    await result.current.loadPhrases("pinyin");
  });
  expect(list).toHaveBeenLastCalledWith(0, 100, "pinyin", "合成");
  expect(result.current.phrases).toEqual([entry]);
  act(() => result.current.setPhraseSearch("无匹配合成词"));
  await act(async () => {
    await result.current.loadPhrases("pinyin");
  });
  expect(result.current.phrases).toEqual([]);
  expect(result.current.phrasePage.status).toBe("未找到匹配词条");
});

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

test("a dictionary failure retry from a replaced client cannot reload stale phrases", async () => {
  let resolveRetry!: () => void;
  const pending = new Promise<void>((resolve) => {
    resolveRetry = resolve;
  });
  const stale = { kind: "quick_phrase" as const, key: "old", value: "旧结果", weight: 1 };
  const oldList = vi.fn().mockResolvedValue({ entries: [stale], has_more: false });
  const oldClient: DictionaryManagerClient = {
    dictionary: {
      list: oldList,
      edit: vi.fn().mockResolvedValue(undefined),
      retry: vi.fn(() => pending),
    },
  };
  const nextList = vi.fn().mockResolvedValue({ entries: [], has_more: false });
  const nextClient: DictionaryManagerClient = {
    dictionary: { list: nextList, edit: vi.fn().mockResolvedValue(undefined) },
  };
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm: vi.fn() }),
    { initialProps: { client: oldClient } },
  );

  let pendingRetry!: Promise<void>;
  act(() => {
    pendingRetry = result.current.retryDictionaryFailure("request-1");
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));
  rerender({ client: nextClient });
  resolveRetry();
  await act(async () => pendingRetry);

  expect(oldList).not.toHaveBeenCalled();
  expect(nextList).not.toHaveBeenCalled();
  expect(result.current.phrases).toEqual([]);
});

test("a dictionary export response from a replaced client cannot publish stale feedback", async () => {
  let resolveExport!: (path: string) => void;
  const pendingExport = new Promise<string>((resolve) => {
    resolveExport = resolve;
  });
  const oldClient: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn(),
      edit: vi.fn().mockResolvedValue(undefined),
      export: vi.fn().mockResolvedValue({ text: "旧词\told", has_more: false }),
    },
    saveExport: vi.fn(() => pendingExport),
  };
  const nextClient: DictionaryManagerClient = {
    dictionary: { list: vi.fn(), edit: vi.fn().mockResolvedValue(undefined) },
    saveExport: vi.fn().mockResolvedValue("/new/export.txt"),
  };
  const { result, rerender } = renderHook(
    ({ client }) => useDictionaryManager({ client, confirm: vi.fn() }),
    { initialProps: { client: oldClient } },
  );

  let pendingExportAction!: Promise<void>;
  act(() => {
    pendingExportAction = result.current.exportPhrases();
  });
  await waitFor(() => expect(oldClient.saveExport).toHaveBeenCalled());
  rerender({ client: nextClient });
  resolveExport("/old/export.txt");
  await act(async () => pendingExportAction);

  expect(result.current.phraseNotice).toBe("");
  expect(result.current.phraseError).toBe("");
});

test.each(["export", "reset"] as const)(
  "a delayed initial query preserves the %s success notice",
  async (action) => {
    vi.useFakeTimers();
    const list = vi.fn().mockResolvedValue({ entries: [], has_more: false });
    const client: DictionaryManagerClient = {
      dictionary: {
        list,
        edit: vi.fn(),
        export: vi.fn().mockResolvedValue({ text: "synthetic\tfixture\t10\n", has_more: false }),
      },
      saveExport: vi.fn().mockResolvedValue("/synthetic/export.txt"),
      resetLearnedData: vi.fn().mockResolvedValue(undefined),
    };
    const { result, unmount } = renderHook(() =>
      useDictionaryManager({ client, confirm: vi.fn().mockResolvedValue(true) }),
    );
    try {
      await act(async () => {
        if (action === "export") await result.current.exportPhrases();
        else await result.current.resetLearnedData();
      });
      // 清除学习数据的结果单独存放，桌面把它画在「学习数据」组里。
      const shown = () =>
        action === "export" ? result.current.phraseNotice : result.current.learnedDataNotice;
      const notice = shown();
      expect(notice).toContain(action === "export" ? "已导出 1 条用户词条" : "已清除所有学习数据");
      expect(list).not.toHaveBeenCalled();
      await act(async () => vi.advanceTimersByTimeAsync(280));
      expect(list).toHaveBeenCalledOnce();
      expect(shown()).toBe(notice);
      act(() => result.current.setPhraseSearch("synthetic-query"));
      expect(shown()).toBe("");
    } finally {
      unmount();
      vi.useRealTimers();
    }
  },
);

test("首次防抖查询保留词条保存错误，查询条件变化才清理", async () => {
  vi.useFakeTimers();
  const list = vi.fn().mockResolvedValue({ entries: [], has_more: false });
  const edit = vi.fn().mockRejectedValue({ code: "dictionary_invalid_entry" });
  const client: DictionaryManagerClient = { dictionary: { list, edit } };
  const { result, unmount } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  try {
    act(() => {
      result.current.setDictionaryKind("pinyin");
      result.current.setPhraseForm({ key: "nhao", value: "合成", weight: 1, previous: null });
    });
    await act(async () => result.current.savePhrase());
    const error = result.current.phraseError;
    expect(error).toContain("完整音节");
    expect(list).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTimeAsync(280));
    expect(list).toHaveBeenCalledExactlyOnceWith(0, 100, "pinyin", "");
    expect(result.current.phraseError).toBe(error);
    act(() => result.current.setPhraseSearch("合成查询"));
    expect(result.current.phraseError).toBe("");
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

test("显式刷新在查询成功后清除旧的读取错误", async () => {
  vi.useFakeTimers();
  const list = vi
    .fn()
    .mockRejectedValueOnce(new Error("synthetic-read-failure"))
    .mockResolvedValueOnce({ entries: [], has_more: false });
  const client: DictionaryManagerClient = { dictionary: { list, edit: vi.fn() } };
  const { result, unmount } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  try {
    await act(async () => vi.advanceTimersByTimeAsync(280));
    expect(result.current.phraseError).toBe("无法读取词库。");
    await act(async () => result.current.loadPhrases());
    expect(list).toHaveBeenCalledTimes(2);
    expect(result.current.phraseError).toBe("");
    expect(result.current.phrasePage.status).toBe("未找到匹配词条");
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

test("词条保存回执不能把切换后的词库列表换回旧类型", async () => {
  vi.useFakeTimers();
  let resolveEdit!: () => void;
  const pendingEdit = new Promise<void>((resolve) => {
    resolveEdit = resolve;
  });
  const pinyin = { kind: "pinyin" as const, key: "he'cheng", value: "合成拼音", weight: 10 };
  const wubi = { kind: "wubi" as const, key: "wgkb", value: "合成五笔", weight: 10 };
  const list = vi.fn(async (_offset: number, _limit: number, kind?: string) => ({
    entries: kind === "pinyin" ? [pinyin] : kind === "wubi" ? [wubi] : [],
    has_more: false,
  }));
  const client: DictionaryManagerClient = {
    dictionary: { list, edit: vi.fn(() => pendingEdit) },
  };
  const { result, unmount } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  try {
    act(() => result.current.setDictionaryKind("pinyin"));
    await act(async () => vi.advanceTimersByTimeAsync(280));
    act(() =>
      result.current.setPhraseForm({
        key: pinyin.key,
        value: pinyin.value,
        weight: 20,
        previous: pinyin,
      }),
    );
    let saving!: Promise<void>;
    act(() => {
      saving = result.current.savePhrase();
    });
    act(() => {
      result.current.setDictionaryKind("wubi");
      result.current.setPhraseForm(null);
      result.current.setPhrases([]);
    });
    await act(async () => vi.advanceTimersByTimeAsync(280));
    expect(result.current.phrases).toEqual([wubi]);
    await act(async () => {
      resolveEdit();
      await saving;
    });
    expect(result.current.dictionaryKind).toBe("wubi");
    expect(result.current.phrases).toEqual([wubi]);
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

test.each(["save", "remove", "import", "retry", "dismiss"] as const)(
  "%s 回执不能覆盖新的搜索结果，后续操作仍可刷新",
  async (action) => {
    vi.useFakeTimers();
    let finish!: () => void;
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const original = { kind: "quick_phrase" as const, key: "old", value: "合成旧词", weight: 10 };
    const current = { ...original, key: "new", value: "合成新词" };
    const list = vi.fn(async (_offset: number, _limit: number, _kind?: string, query?: string) => ({
      entries: query === "new" ? [current] : [original],
      has_more: false,
    }));
    const client: DictionaryManagerClient = {
      dictionary: {
        list,
        edit: vi.fn(() => pending),
        import: vi.fn(async () => {
          await pending;
          return { applied: 1, failed: 0 };
        }),
        retry: vi.fn(() => pending),
        dismissFailure: vi.fn(() => pending),
      },
    };
    const { result, unmount } = renderHook(() =>
      useDictionaryManager({
        client,
        confirm: vi.fn().mockResolvedValue(true),
      }),
    );
    try {
      await act(async () => vi.advanceTimersByTimeAsync(280));
      act(() => result.current.setPhraseForm({ ...original, previous: original }));
      let operation!: Promise<void>;
      await act(async () => {
        switch (action) {
          case "save":
            operation = result.current.savePhrase();
            break;
          case "remove":
            operation = result.current.removePhrase(original);
            break;
          case "import":
            operation = result.current.importPhrases(
              new File(["合成词\tfixture\t10\n"], "synthetic.txt"),
              "standard",
            );
            break;
          case "retry":
            operation = result.current.retryDictionaryFailure("synthetic-request");
            break;
          case "dismiss":
            operation = result.current.dismissDictionaryFailure("synthetic-request");
            break;
        }
      });
      // 文件读取完成后再切条件，确保导入请求也已开始等待回执。
      if (action === "import") {
        await vi.waitFor(() => expect(client.dictionary!.import).toHaveBeenCalled());
      }
      act(() => result.current.setPhraseSearch("new"));
      await act(async () => vi.advanceTimersByTimeAsync(280));
      expect(result.current.phrases).toEqual([current]);
      await act(async () => {
        finish();
        await operation;
      });
      expect(result.current.phrases).toEqual([current]);
      expect(result.current.phraseBusy).toBe(false);
      await act(async () => result.current.retryDictionaryFailure("synthetic-next"));
      expect(list).toHaveBeenLastCalledWith(0, 100, "quick_phrase", "new");
      expect(result.current.phrases).toEqual([current]);
    } finally {
      unmount();
      vi.useRealTimers();
    }
  },
);

test("旧保存回执刷新当前搜索并显示刚提交的匹配词条", async () => {
  vi.useFakeTimers();
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  let committed = false;
  const current = { kind: "quick_phrase" as const, key: "new", value: "合成新词", weight: 10 };
  const list = vi.fn(async (_offset: number, _limit: number, _kind?: string, query?: string) => ({
    entries: committed && query === "new" ? [current] : [],
    has_more: false,
  }));
  const client: DictionaryManagerClient = {
    dictionary: {
      list,
      edit: vi.fn(async () => {
        await pending;
        committed = true;
      }),
    },
  };
  const { result, unmount } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  try {
    await act(async () => vi.advanceTimersByTimeAsync(280));
    act(() => result.current.setPhraseForm({ ...current, previous: null }));
    let saving!: Promise<void>;
    act(() => {
      saving = result.current.savePhrase();
    });
    act(() => result.current.setPhraseSearch("new"));
    await act(async () => vi.advanceTimersByTimeAsync(280));
    expect(list).toHaveBeenLastCalledWith(0, 100, "quick_phrase", "new");
    expect(result.current.phrases).toEqual([]);
    await act(async () => {
      finish();
      await saving;
    });
    expect(result.current.phraseSearch).toBe("new");
    expect(result.current.phrases).toEqual([current]);
    expect(result.current.phraseBusy).toBe(false);
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

test("新搜索首页等待时旧保存回执不能沿用上个搜索的页码", async () => {
  vi.useFakeTimers();
  let finishEdit!: () => void;
  const pendingEdit = new Promise<void>((resolve) => {
    finishEdit = resolve;
  });
  let finishSearch!: () => void;
  const pendingSearch = new Promise<void>((resolve) => {
    finishSearch = resolve;
  });
  const original = { kind: "quick_phrase" as const, key: "old", value: "合成旧词", weight: 10 };
  const current = { ...original, key: "new", value: "合成新词" };
  const list = vi.fn(async (offset: number, _limit: number, _kind?: string, query?: string) => {
    if (query !== "new") return { entries: [original], has_more: true };
    if (offset > 0) return { entries: [], has_more: false };
    await pendingSearch;
    return { entries: [current], has_more: false };
  });
  const client: DictionaryManagerClient = {
    dictionary: { list, edit: vi.fn(() => pendingEdit) },
  };
  const { result, unmount } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  try {
    await act(async () => vi.advanceTimersByTimeAsync(280));
    await act(async () => result.current.turnPhrasePage(100));
    expect(result.current.phrasePage.offset).toBe(100);
    act(() => result.current.setPhraseForm({ ...current, previous: null }));
    let saving!: Promise<void>;
    act(() => {
      saving = result.current.savePhrase();
    });
    act(() => result.current.setPhraseSearch("new"));
    await act(async () => vi.advanceTimersByTimeAsync(280));
    expect(list).toHaveBeenLastCalledWith(0, 100, "quick_phrase", "new");
    await act(async () => {
      finishEdit();
      await Promise.resolve();
    });
    await act(async () => {
      finishSearch();
      await saving;
    });
    expect(list).toHaveBeenLastCalledWith(0, 100, "quick_phrase", "new");
    expect(result.current.phrasePage.offset).toBe(0);
    expect(result.current.phrases).toEqual([current]);
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

test.each(["windows", "rime"] as const)("桌面导出保留 %s 格式与分页参数", async (format) => {
  const text = format === "windows" ? "code\t合成\t2\n" : "合成\tcode\t2\n";
  const exportPage = vi
    .fn()
    .mockResolvedValueOnce({ text, has_more: true })
    .mockResolvedValueOnce({ text: "", has_more: true });
  const saveExport = vi.fn().mockResolvedValue("/synthetic/export.txt");
  const client: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit: vi.fn(),
      export: exportPage,
    },
    saveExport,
  };
  const { result } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  act(() => {
    result.current.setDictionaryKind("wubi98");
    result.current.setDictionaryFormat(format);
  });
  await act(async () => result.current.exportPhrases());
  expect(exportPage.mock.calls).toEqual([
    ["wubi98", format === "rime" ? "standard" : format, 0, 1000],
    ["wubi98", format === "rime" ? "standard" : format, 1, 1000],
  ]);
  expect(saveExport).toHaveBeenCalledWith("水杉IME-98五笔用户词库.txt", "\ufeff" + text);
  expect(result.current.phraseNotice).toContain("已导出 1 条用户词条");
});

test("桌面逐页检查原始文本上限，过滤前超限也不能保存", async () => {
  const exportPage = vi.fn().mockResolvedValue({
    text: "甲\t" + "a".repeat(32 * 1024 * 1024) + "\n合成\thecheng\t2\n",
    has_more: true,
  });
  const saveExport = vi.fn();
  const client: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit: vi.fn(),
      export: exportPage,
    },
    saveExport,
  };
  const { result } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  act(() => result.current.setDictionaryKind("pinyin"));
  await act(async () => result.current.exportPhrases());
  expect(exportPage).toHaveBeenCalledOnce();
  expect(saveExport).not.toHaveBeenCalled();
  expect(result.current.phraseError).toBe("词库导出失败，请稍后重试。");
});

test("桌面分页导出保留保存取消提示", async () => {
  const saveExport = vi.fn().mockResolvedValue(null);
  const client: DictionaryManagerClient = {
    dictionary: {
      list: vi.fn(),
      edit: vi.fn(),
      export: vi.fn().mockResolvedValue({ text: "合成\tcode\t2\n", has_more: false }),
    },
    saveExport,
  };
  const { result } = renderHook(() => useDictionaryManager({ client, confirm: vi.fn() }));
  await act(async () => result.current.exportPhrases());
  expect(saveExport).toHaveBeenCalledOnce();
  expect(result.current.phraseNotice).toBe("已取消导出。");
  expect(result.current.phraseError).toBe("");
});
