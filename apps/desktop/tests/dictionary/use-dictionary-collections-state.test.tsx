// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useDictionaryCollectionsState,
  type UseDictionaryCollectionsStateOptions,
} from "../../../../packages/ui/src/dictionary/use-dictionary-collections-state";
import type {
  DictionaryCollectionsClient,
  DictionaryCollectionsView,
} from "../../../../packages/ui/src/dictionary/dictionary-collections";

afterEach(cleanup);

function view(name = "合成词库"): DictionaryCollectionsView {
  return {
    collections: [
      {
        id: "synthetic",
        name,
        kind: "pinyin",
        source: { type: "user" },
        enabled: true,
        entry_count: 1,
        pending: 0,
      },
    ],
    formats: ["txt"],
  };
}

function client() {
  const initial = view();
  return {
    load: vi.fn().mockResolvedValue(initial),
    flush: vi.fn().mockResolvedValue(initial),
    create: vi.fn().mockResolvedValue(initial),
    delete: vi.fn().mockResolvedValue(initial),
    setEnabled: vi.fn().mockResolvedValue(initial),
    addWords: vi.fn().mockResolvedValue(initial),
    importFile: vi.fn().mockResolvedValue(initial),
    installCommunity: vi.fn().mockResolvedValue(initial),
  } satisfies DictionaryCollectionsClient;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function setup(overrides: Partial<UseDictionaryCollectionsStateOptions> = {}, strict = false) {
  const collections = client();
  const dictionary = { count: vi.fn().mockResolvedValue(12) };
  const onNotice = vi.fn();
  const options: UseDictionaryCollectionsStateOptions = {
    collections,
    dictionary,
    onNotice,
    galleryOpen: false,
    ...overrides,
  };
  const hook = renderHook((props) => useDictionaryCollectionsState(props), {
    initialProps: options,
    reactStrictMode: strict,
  });
  return { ...hook, collections, dictionary, onNotice, options };
}

test("脱离设置上下文在 StrictMode 下加载列表和计数，修改后发布视图与提示", async () => {
  const { result, collections, dictionary, onNotice } = setup({}, true);
  await waitFor(() => expect(result.current.view).toEqual(view()));
  expect(result.current.builtinCount).toBe(12);
  expect(dictionary.count).toHaveBeenCalledWith("pinyin");
  expect(collections.flush).toHaveBeenCalledTimes(2);
  const changed = view("已修改");
  await act(async () =>
    expect(
      await result.current.run(
        async () => changed,
        () => "合成完成",
      ),
    ).toBe(changed),
  );
  expect(result.current.view).toBe(changed);
  expect(result.current.busy).toBe(false);
  expect(onNotice.mock.calls).toEqual([["合成完成"]]);
});

test("图库盖住页面时不发起刷新，关闭后重新同步", async () => {
  const { result, collections, options, rerender } = setup({ galleryOpen: true });
  expect(collections.flush).not.toHaveBeenCalled();
  expect(result.current.view).toBeNull();
  rerender({ ...options, galleryOpen: false });
  await waitFor(() => expect(result.current.view).toEqual(view()));
  expect(collections.flush).toHaveBeenCalledOnce();
});

test.each(["成功", "失败"])("图库打开隔离旧列表的%s和旧计数，但保留修改归属", async (outcome) => {
  const refresh = deferred<DictionaryCollectionsView>();
  const count = deferred<number>();
  const collections = client();
  collections.flush.mockReturnValueOnce(refresh.promise);
  const { result, options, rerender, onNotice } = setup({
    collections,
    dictionary: { count: () => count.promise },
  });
  const guard = result.current.captureOwner();
  rerender({ ...options, galleryOpen: true });
  expect(guard()).toBe(true);
  await act(async () => {
    count.resolve(99);
    if (outcome === "成功") refresh.resolve(view("过期"));
    else refresh.reject(new Error("synthetic stale refresh"));
  });
  expect(result.current.view).toBeNull();
  expect(result.current.builtinCount).toBeNull();
  expect(result.current.loadFailure).toBe("");
  await act(async () => result.current.run(async () => view("当前修改"), "已修改"));
  expect(result.current.view).toEqual(view("当前修改"));
  expect(onNotice).toHaveBeenCalledWith("已修改");
});

test("同轮重复修改只执行一次，修改取得列表和计数所有权", async () => {
  const refresh = deferred<DictionaryCollectionsView>();
  const count = deferred<number>();
  const mutation = deferred<DictionaryCollectionsView>();
  const collections = client();
  collections.flush.mockReturnValueOnce(refresh.promise);
  const { result, onNotice } = setup({ collections, dictionary: { count: () => count.promise } });
  const action = vi.fn(() => mutation.promise);
  let pending!: Promise<DictionaryCollectionsView | undefined>;
  let duplicate!: Promise<DictionaryCollectionsView | undefined>;
  act(() => {
    pending = result.current.run(action, "已修改");
    duplicate = result.current.run(action, "重复");
  });
  expect(action).toHaveBeenCalledOnce();
  expect(result.current.busy).toBe(true);
  await act(async () => {
    mutation.resolve(view("当前"));
    await pending;
    await duplicate;
    refresh.resolve(view("过期"));
    count.resolve(99);
  });
  expect(result.current.view).toEqual(view("当前"));
  expect(result.current.builtinCount).toBeNull();
  expect(result.current.busy).toBe(false);
  expect(onNotice.mock.calls).toEqual([["已修改"]]);
});

test.each(["成功", "失败"])("更换客户端后旧修改%s不影响新列表和新忙碌", async (outcome) => {
  const { result, options, rerender, onNotice } = setup();
  await waitFor(() => expect(result.current.view).toEqual(view()));
  const old = deferred<DictionaryCollectionsView>();
  const next = deferred<DictionaryCollectionsView>();
  let oldRun!: Promise<DictionaryCollectionsView | undefined>;
  let nextRun!: Promise<DictionaryCollectionsView | undefined>;
  act(() => {
    oldRun = result.current.run(() => old.promise, "旧提示");
  });
  const guard = result.current.captureOwner();
  const replacement = client();
  replacement.flush.mockResolvedValue(view("新列表"));
  rerender({ ...options, collections: replacement });
  expect(guard()).toBe(false);
  await waitFor(() => expect(result.current.view).toEqual(view("新列表")));
  act(() => {
    nextRun = result.current.run(() => next.promise, "新提示");
  });
  await act(async () => {
    if (outcome === "成功") old.resolve(view("过期"));
    else old.reject(new Error("synthetic stale mutation"));
    expect(await oldRun).toBeUndefined();
  });
  expect(result.current.busy).toBe(true);
  expect(result.current.view).toEqual(view("新列表"));
  expect(replacement.load).not.toHaveBeenCalled();
  expect(onNotice).not.toHaveBeenCalled();
  await act(async () => {
    next.resolve(view("新修改"));
    await nextRun;
  });
  expect(result.current.busy).toBe(false);
  expect(result.current.view).toEqual(view("新修改"));
  expect(onNotice.mock.calls).toEqual([["新提示"]]);
});

test("当前修改失败报告错误并重新读取，不再 flush 队列", async () => {
  const { result, collections, onNotice } = setup();
  await waitFor(() => expect(result.current.view).toEqual(view()));
  collections.load.mockResolvedValue(view("恢复列表"));
  await act(async () =>
    expect(
      await result.current.run(async () => {
        throw { code: "collections_not_found" };
      }),
    ).toBeUndefined(),
  );
  await waitFor(() => expect(result.current.view).toEqual(view("恢复列表")));
  expect(collections.load).toHaveBeenCalledOnce();
  expect(collections.flush).toHaveBeenCalledOnce();
  expect(onNotice.mock.calls).toEqual([["这个词库已经不在了，列表已刷新。"]]);
});

test("读取失败保留错误，重新打开图库后成功刷新会清理它", async () => {
  const collections = client();
  collections.flush.mockRejectedValueOnce({ code: "collections_corrupt" });
  const { result, options, rerender, onNotice } = setup({ collections });
  await waitFor(() =>
    expect(result.current.loadFailure).toBe("词库文件已损坏，为避免丢失没有改动它。"),
  );
  rerender({ ...options, galleryOpen: true });
  rerender({ ...options, galleryOpen: false });
  await waitFor(() => expect(result.current.view).toEqual(view()));
  expect(result.current.loadFailure).toBe("");
  expect(onNotice).not.toHaveBeenCalled();
});

test("没有集合能力时仍可读取内置计数，计数失败不产生词库错误", async () => {
  const { result } = setup({ collections: undefined });
  await waitFor(() => expect(result.current.builtinCount).toBe(12));
  expect(result.current.view).toBeNull();
  const count = vi.fn().mockRejectedValue(new Error("synthetic count failure"));
  const failed = setup({ collections: undefined, dictionary: { count } });
  await act(async () => {
    await Promise.resolve();
  });
  expect(failed.result.current.builtinCount).toBeNull();
  expect(failed.result.current.loadFailure).toBe("");
  expect(failed.onNotice).not.toHaveBeenCalled();
});

test("准备阶段按调用时捕获归属，更换任一客户端或卸载后失效", async () => {
  const { result, options, rerender, unmount } = setup();
  await waitFor(() => expect(result.current.view).toEqual(view()));
  let guard = result.current.captureOwner();
  expect(guard()).toBe(true);
  const replacementDictionary = { count: async () => 13 };
  rerender({ ...options, dictionary: replacementDictionary });
  expect(guard()).toBe(false);
  guard = result.current.captureOwner();
  expect(guard()).toBe(true);
  rerender({ ...options, dictionary: replacementDictionary, community: {} });
  expect(guard()).toBe(false);
  guard = result.current.captureOwner();
  expect(guard()).toBe(true);
  unmount();
  expect(guard()).toBe(false);
});
