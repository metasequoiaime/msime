// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { DictionaryClient } from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import type { DictionaryEntry } from "../../../../packages/ui/src/dictionary/dictionary-file";
import { useBuiltinDictionaryEntries } from "../../../../packages/ui/src/dictionary/use-builtin-dictionary-entries";

type Page = Awaited<ReturnType<DictionaryClient["list"]>>;

function entry(value: string): DictionaryEntry {
  return { kind: "pinyin", key: "he'cheng", value, weight: 10 };
}

function deferred() {
  let resolve!: (page: Page) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<Page>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

async function advance(ms = 1) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("脱离设置上下文初始化，StrictMode 重放清理旧定时器并保留方法接收者", async () => {
  const dictionary = {
    marker: "synthetic",
    list: vi.fn<DictionaryClient["list"]>(async function (this: { marker: string }) {
      expect(this.marker).toBe("synthetic");
      return { entries: [entry("合成首词")], has_more: true };
    }),
  };
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary }), {
    reactStrictMode: true,
  });
  expect(result.current.loading).toBe(true);
  expect(dictionary.list).not.toHaveBeenCalled();
  await advance();
  expect(dictionary.list).toHaveBeenCalledExactlyOnceWith(0, 100, "pinyin", undefined);
  expect(result.current.entries).toEqual([entry("合成首词")]);
  expect(result.current.more).toBe(true);
  expect(result.current.loading).toBe(false);
});

test("拼音查询规范化并防抖，清空查询在下一轮定时器读取", async () => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({ entries: [], has_more: false })),
  };
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary }));
  await advance();
  act(() => result.current.setQuery("He Cheng"));
  await advance(249);
  expect(dictionary.list).toHaveBeenCalledTimes(1);
  await advance();
  expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "he'cheng");
  act(() => result.current.setQuery(""));
  await advance();
  expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", undefined);
  expect(dictionary.list).toHaveBeenCalledTimes(3);
});

test("词语过滤使用原始页条数推进偏移，整页没有匹配仍能继续", async () => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({ entries: [], has_more: false })),
  };
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary }));
  await advance();
  dictionary.list.mockResolvedValueOnce({
    entries: [entry("甲"), entry("合成首词"), entry("乙")],
    has_more: true,
  });
  act(() => result.current.setQuery(" 合成 "));
  await advance(250);
  expect(result.current.entries).toEqual([entry("合成首词")]);
  dictionary.list.mockResolvedValueOnce({ entries: [entry("丙"), entry("丁")], has_more: true });
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(3, 100, "pinyin", undefined);
  expect(result.current.entries).toEqual([entry("合成首词")]);
  dictionary.list.mockResolvedValueOnce({ entries: [entry("合成末词")], has_more: false });
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(5, 100, "pinyin", undefined);
  expect(result.current.entries).toEqual([entry("合成首词"), entry("合成末词")]);
  expect(result.current.more).toBe(false);
});

test("刷新替换已有分页并重置后续偏移", async () => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({
      entries: [entry("首词"), entry("次词")],
      has_more: true,
    })),
  };
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary }));
  await advance();
  dictionary.list.mockResolvedValueOnce({ entries: [entry("末词")], has_more: false });
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(2, 100, "pinyin", undefined);
  dictionary.list.mockResolvedValueOnce({ entries: [entry("新首词")], has_more: true });
  await act(async () => {
    await result.current.reload();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", undefined);
  expect(result.current.entries).toEqual([entry("新首词")]);
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(1, 100, "pinyin", undefined);
});

test.each(["成功", "失败"])("搜索防抖期间丢弃旧请求的%s", async (outcome) => {
  const old = deferred();
  const dictionary = {
    list: vi
      .fn<DictionaryClient["list"]>(async () => ({ entries: [entry("新词")], has_more: false }))
      .mockReturnValueOnce(old.promise),
  };
  const onNotice = vi.fn();
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary, onNotice }));
  await advance();
  act(() => result.current.setQuery("xin"));
  await act(async () => {
    if (outcome === "成功") old.resolve({ entries: [entry("旧词")], has_more: true });
    else old.reject(new Error("synthetic old query"));
  });
  expect(result.current.entries).toEqual([]);
  expect(result.current.loading).toBe(true);
  expect(onNotice).not.toHaveBeenCalled();
  await advance(250);
  expect(result.current.entries).toEqual([entry("新词")]);
});

test.each(["成功", "失败"])("旧分页的%s不覆盖更新的刷新结果或释放新请求", async (outcome) => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({
      entries: [entry("首词")],
      has_more: true,
    })),
  };
  const onNotice = vi.fn();
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary, onNotice }));
  await advance();
  const old = deferred();
  const fresh = deferred();
  dictionary.list.mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise);
  act(() => {
    void result.current.loadMore();
  });
  act(() => {
    void result.current.reload();
  });
  await act(async () => {
    if (outcome === "成功")
      old.resolve({ entries: [entry("旧词"), entry("旧次词")], has_more: false });
    else old.reject(new Error("synthetic old page"));
  });
  expect(result.current.loading).toBe(true);
  expect(result.current.entries).toEqual([entry("首词")]);
  expect(onNotice).not.toHaveBeenCalled();
  await act(async () => fresh.resolve({ entries: [entry("新首词")], has_more: true }));
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(1, 100, "pinyin", undefined);
  expect(result.current.entries).toEqual([entry("新首词"), entry("首词")]);
});

test("客户端变化作废旧请求并从零读取新客户端", async () => {
  const old = deferred();
  const first = { list: vi.fn<DictionaryClient["list"]>(() => old.promise) };
  const second = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({
      entries: [entry("新词")],
      has_more: true,
    })),
  };
  const { result, rerender } = renderHook(
    ({ dictionary }) => useBuiltinDictionaryEntries({ dictionary }),
    { initialProps: { dictionary: first } },
  );
  await advance();
  rerender({ dictionary: second });
  await act(async () => old.resolve({ entries: [entry("旧词")], has_more: true }));
  await advance();
  expect(result.current.entries).toEqual([entry("新词")]);
  await act(async () => {
    await result.current.loadMore();
  });
  expect(second.list).toHaveBeenLastCalledWith(1, 100, "pinyin", undefined);
});

test("失败分页保留词条与偏移，重试使用最新提示回调", async () => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({
      entries: [entry("首词")],
      has_more: true,
    })),
  };
  const first = vi.fn();
  const second = vi.fn();
  const pending = deferred();
  const { result, rerender } = renderHook(
    ({ onNotice }) => useBuiltinDictionaryEntries({ dictionary, onNotice }),
    { initialProps: { onNotice: first } },
  );
  await advance();
  dictionary.list.mockReturnValueOnce(pending.promise);
  act(() => {
    void result.current.loadMore();
  });
  rerender({ onNotice: second });
  await act(async () => pending.reject(new Error("synthetic page failure")));
  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledExactlyOnceWith("读取词条失败，请稍后重试。");
  expect(result.current.entries).toEqual([entry("首词")]);
  expect(result.current.loading).toBe(false);
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(1, 100, "pinyin", undefined);
});

test("搜索词改变后立刻加载更多不会沿用旧查询的偏移", async () => {
  const dictionary = {
    list: vi.fn<DictionaryClient["list"]>(async () => ({
      entries: [entry("首词"), entry("次词")],
      has_more: true,
    })),
  };
  const { result } = renderHook(() => useBuiltinDictionaryEntries({ dictionary }));
  await advance();
  act(() => result.current.setQuery("xin"));
  await act(async () => {
    await result.current.loadMore();
  });
  expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "xin");
});

test.each(["成功", "失败"])("卸载后丢弃迟到的%s并取消未执行的搜索", async (outcome) => {
  const pending = deferred();
  const dictionary = { list: vi.fn<DictionaryClient["list"]>(() => pending.promise) };
  const onNotice = vi.fn();
  const { result, unmount } = renderHook(() =>
    useBuiltinDictionaryEntries({ dictionary, onNotice }),
  );
  await advance();
  act(() => result.current.setQuery("xin"));
  unmount();
  await act(async () => {
    if (outcome === "成功") pending.resolve({ entries: [entry("旧词")], has_more: true });
    else pending.reject(new Error("synthetic unmounted query"));
    await vi.advanceTimersByTimeAsync(250);
  });
  expect(onNotice).not.toHaveBeenCalled();
  expect(dictionary.list).toHaveBeenCalledOnce();
});
