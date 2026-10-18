// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useCloudDictionaryCatalogPanel, type CloudDictionaryPanelClient } from "@msime/ui";

type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
const entry = { kind: "pinyin" as const, code: "synthetic", word: "合成目录词条", weight: 10 };
const page: Response = {
  catalog_entries: [entry],
  revision: 7,
  offset: 0,
  has_more: true,
  normalized: "synthetic-normalized",
};
function host(request = vi.fn().mockResolvedValue(page)): CloudDictionaryPanelClient {
  return { request, close: vi.fn() };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
afterEach(cleanup);

test("StrictMode 不自动发送查询，分页使用已确认参数而非新草稿", async () => {
  const client = host();
  const { result } = renderHook(() => useCloudDictionaryCatalogPanel(client), {
    reactStrictMode: true,
  });
  expect(client.request).not.toHaveBeenCalled();
  await act(async () => {
    await result.current.queryCatalog();
  });
  expect(result.current.notice).toBe("请输入查询编码");
  act(() => {
    result.current.setCode(" synthetic ");
    result.current.setScheme("shuangpin");
    result.current.setProfile("ziranma");
  });
  await act(async () => {
    await result.current.queryCatalog();
  });
  expect(client.request).toHaveBeenLastCalledWith({
    operation: "catalog",
    kind: "pinyin",
    code: "synthetic",
    scheme: "shuangpin",
    profile: "ziranma",
    offset: 0,
  });
  const confirmed = result.current.confirmed!;
  expect(result.current.normalized).toBe("synthetic-normalized");
  act(() => {
    result.current.setCode("synthetic-draft");
    result.current.setScheme("pinyin");
  });
  await act(async () => {
    await result.current.queryCatalog(100, confirmed);
  });
  expect(client.request).toHaveBeenLastCalledWith({
    operation: "catalog",
    kind: "pinyin",
    ...confirmed,
    offset: 100,
  });
});

test("类型切换清理结果与编辑，快捷短语允许空编码且不自动查询", async () => {
  const client = host();
  const { result } = renderHook(() => useCloudDictionaryCatalogPanel(client));
  act(() => result.current.setCode("synthetic"));
  await act(async () => {
    await result.current.queryCatalog();
  });
  act(() => result.current.beginEdit(entry));
  act(() => result.current.changeKind("quick"));
  expect(result.current.entries).toEqual([]);
  expect(result.current.form).toBeNull();
  expect(result.current.confirmed).toBeNull();
  expect(client.request).toHaveBeenCalledTimes(1);
  act(() => result.current.setCode(""));
  await act(async () => {
    await result.current.queryCatalog();
  });
  expect(client.request).toHaveBeenLastCalledWith({
    operation: "catalog",
    kind: "quick",
    code: "",
    scheme: "pinyin",
    profile: "xiaohe",
    offset: 0,
  });
});

test("保存使用原词条和目录修订号，防重复并按已确认查询刷新", async () => {
  const saved = deferred<Response>();
  const request = vi
    .fn()
    .mockResolvedValueOnce(page)
    .mockReturnValueOnce(saved.promise)
    .mockResolvedValue(page);
  const client = host(request);
  const { result } = renderHook(() => useCloudDictionaryCatalogPanel(client));
  act(() => result.current.setCode("synthetic"));
  await act(async () => {
    await result.current.queryCatalog();
  });
  act(() => result.current.beginEdit(entry));
  act(() => {
    result.current.changeForm({ code: " replacement ", word: " 合成更新 ", weight: 42 });
    result.current.setCode("synthetic-draft");
  });
  act(() => {
    void result.current.save();
    void result.current.save();
    result.current.changeKind("quick");
  });
  expect(request).toHaveBeenCalledTimes(2);
  expect(result.current.kind).toBe("pinyin");
  expect(request).toHaveBeenLastCalledWith({
    operation: "edit_catalog",
    kind: "pinyin",
    code: entry.code,
    word: entry.word,
    revision: 7,
    replacement: { code: "replacement", word: " 合成更新 ", weight: 42 },
  });
  await act(async () => saved.resolve({}));
  expect(result.current.form).toBeNull();
  expect(result.current.notice).toBe("完整目录已保存");
  expect(request).toHaveBeenLastCalledWith({
    operation: "catalog",
    kind: "pinyin",
    code: "synthetic",
    scheme: "pinyin",
    profile: "xiaohe",
    offset: 0,
  });
});

test("保存成功但刷新失败时清理旧结果并保留成功提示", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce(page)
    .mockResolvedValueOnce({})
    .mockRejectedValueOnce(new Error("synthetic failure"));
  const client = host(request);
  const { result } = renderHook(() => useCloudDictionaryCatalogPanel(client));
  act(() => result.current.setCode("synthetic"));
  await act(async () => {
    await result.current.queryCatalog();
  });
  act(() => result.current.beginEdit(entry));
  await act(async () => {
    await result.current.save();
  });
  expect(result.current.notice).toBe("完整目录已保存，但目录刷新失败，请重新查询");
  expect(result.current.entries).toEqual([]);
  expect(result.current.confirmed).toBeNull();
  expect(result.current.revision).toBe(0);
  expect(result.current.normalized).toBe("");
});

test.each(["resolve", "reject"] as const)(
  "旧客户端查询的 %s 不覆盖新查询或解除新忙碌",
  async (outcome) => {
    const old = deferred<Response>();
    const current = deferred<Response>();
    const first = host(vi.fn().mockReturnValue(old.promise));
    const next = host(vi.fn().mockReturnValue(current.promise));
    const { result, rerender } = renderHook(
      ({ client }) => useCloudDictionaryCatalogPanel(client),
      { initialProps: { client: first } },
    );
    act(() => result.current.setCode("synthetic"));
    act(() => {
      void result.current.queryCatalog();
    });
    rerender({ client: next });
    act(() => {
      void result.current.queryCatalog();
    });
    await act(async () => {
      if (outcome === "resolve") old.resolve(page);
      else old.reject(new Error("synthetic failure"));
    });
    expect(result.current.entries).toEqual([]);
    expect(result.current.busy).toBe(true);
    await act(async () => current.resolve({ ...page, revision: 9 }));
    expect(result.current.entries).toEqual([entry]);
    expect(result.current.revision).toBe(9);
    expect(result.current.busy).toBe(false);
    expect(result.current.notice).toBe("完整目录已刷新");
  },
);

test("旧保存不清理新客户端编辑，取消只丢弃当前表单", async () => {
  const saved = deferred<Response>();
  const first = host(vi.fn().mockResolvedValueOnce(page).mockReturnValueOnce(saved.promise));
  const next = host();
  const { result, rerender } = renderHook(({ client }) => useCloudDictionaryCatalogPanel(client), {
    initialProps: { client: first },
  });
  act(() => result.current.setCode("synthetic"));
  await act(async () => {
    await result.current.queryCatalog();
  });
  act(() => result.current.beginEdit(entry));
  act(() => {
    void result.current.save();
  });
  rerender({ client: next });
  await act(async () => {
    await result.current.queryCatalog();
  });
  act(() => result.current.beginEdit(entry));
  act(() => result.current.changeForm({ word: "合成新编辑" }));
  await act(async () => saved.resolve({}));
  expect(result.current.form?.word).toBe("合成新编辑");
  expect(result.current.notice).toBe("完整目录已刷新");
  act(() => result.current.cancelForm());
  expect(result.current.form).toBeNull();
});
