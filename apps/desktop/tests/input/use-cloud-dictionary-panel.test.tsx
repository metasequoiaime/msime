// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useCloudDictionaryPanel, type CloudDictionaryPanelClient } from "@msime/ui";

const entry = {
  id: "synthetic-entry",
  kind: "pinyin" as const,
  code: "synthetic",
  word: "合成词条",
  weight: 12,
  revision: 3,
};
const list = { entries: [entry], offset: 0, has_more: true };
function client(request = vi.fn().mockResolvedValue(list)): CloudDictionaryPanelClient {
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
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("独立面板在 StrictMode 初始化一次并使用最新搜索分页和切换类型", async () => {
  const host = client();
  const { result } = renderHook(() => useCloudDictionaryPanel(host), { reactStrictMode: true });
  await waitFor(() => expect(result.current.busy).toBe(false));
  await waitFor(() => expect(result.current.entries).toEqual([entry]));
  expect(host.request).toHaveBeenCalledTimes(1);
  act(() => result.current.setSearch("合成查询"));
  expect(host.request).toHaveBeenCalledTimes(1);
  await act(async () => {
    await result.current.refresh(100);
  });
  expect(host.request).toHaveBeenLastCalledWith({
    operation: "list",
    kind: "pinyin",
    offset: 100,
    search: "合成查询",
  });
  await act(async () => result.current.changeKind("wubi"));
  expect(host.request).toHaveBeenLastCalledWith({
    operation: "list",
    kind: "wubi",
    offset: 0,
    search: "合成查询",
  });
});

test("嵌入查询延迟 280ms 且卸载清除 StrictMode 与搜索计时器", async () => {
  vi.useFakeTimers();
  const host = client();
  const { result, unmount } = renderHook(() => useCloudDictionaryPanel(host, true), {
    reactStrictMode: true,
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(279);
  });
  expect(host.request).not.toHaveBeenCalled();
  act(() => result.current.setSearch("合成最新查询"));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(279);
  });
  expect(host.request).not.toHaveBeenCalled();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(host.request).toHaveBeenCalledExactlyOnceWith({
    operation: "list",
    kind: "pinyin",
    offset: 0,
    search: "合成最新查询",
  });
  act(() => result.current.changeKind("quick"));
  unmount();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(280);
  });
  expect(host.request).toHaveBeenCalledTimes(1);
});

test.each(["resolve", "reject"] as const)("客户端更换隔离旧列表的 %s", async (outcome) => {
  const old = deferred<typeof list>();
  const first = client(vi.fn().mockReturnValue(old.promise));
  const next = client();
  const { result, rerender } = renderHook(({ host }) => useCloudDictionaryPanel(host), {
    initialProps: { host: first },
  });
  await waitFor(() => expect(first.request).toHaveBeenCalled());
  rerender({ host: next });
  await waitFor(() => expect(result.current.entries).toEqual([entry]));
  await act(async () => {
    if (outcome === "resolve") old.resolve({ ...list, entries: [] });
    else old.reject(new Error("synthetic failure"));
  });
  expect(result.current.entries).toEqual([entry]);
  expect(result.current.notice).toBe("云词库已刷新");
  expect(result.current.busy).toBe(false);
});

test("新增保留词条空格并同步拒绝重复保存，刷新失败保留成功提示", async () => {
  const saved = deferred<typeof list>();
  const request = vi
    .fn()
    .mockResolvedValueOnce(list)
    .mockReturnValueOnce(saved.promise)
    .mockRejectedValueOnce(new Error("synthetic failure"));
  const host = client(request);
  const { result } = renderHook(() => useCloudDictionaryPanel(host));
  await waitFor(() => expect(result.current.entries).toEqual([entry]));
  act(() => result.current.beginAdd());
  await act(async () => {
    await result.current.save();
  });
  expect(result.current.notice).toBe("编码和词条不能为空");
  act(() => result.current.changeForm({ code: " synthetic ", word: " 合成词条 ", weight: 42 }));
  act(() => {
    void result.current.save();
    void result.current.save();
  });
  expect(request).toHaveBeenCalledTimes(2);
  expect(request).toHaveBeenLastCalledWith({
    operation: "add",
    kind: "pinyin",
    code: "synthetic",
    word: " 合成词条 ",
    weight: 42,
  });
  await act(async () => saved.resolve(list));
  expect(result.current.form).toBeNull();
  expect(result.current.entries).toEqual([]);
  expect(result.current.notice).toBe("云词条已保存，但列表刷新失败，请重新查询");
});

test("编辑透传词条修订号并在客户端更换后丢弃迟到保存", async () => {
  const saved = deferred<typeof list>();
  const first = client(vi.fn().mockResolvedValueOnce(list).mockReturnValueOnce(saved.promise));
  const next = client();
  const { result, rerender } = renderHook(({ host }) => useCloudDictionaryPanel(host), {
    initialProps: { host: first },
  });
  await waitFor(() => expect(result.current.entries).toEqual([entry]));
  act(() => result.current.beginEdit(entry));
  act(() => result.current.changeForm({ word: "合成更新" }));
  act(() => {
    void result.current.save();
  });
  expect(first.request).toHaveBeenLastCalledWith({
    operation: "update",
    kind: "pinyin",
    id: entry.id,
    revision: 3,
    code: "synthetic",
    word: "合成更新",
    weight: 12,
  });
  rerender({ host: next });
  await waitFor(() => expect(next.request).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.beginAdd());
  await act(async () => saved.resolve(list));
  expect(result.current.form?.entry).toBeNull();
  expect(result.current.notice).toBe("云词库已刷新");
  expect(first.request).toHaveBeenCalledTimes(2);
});
