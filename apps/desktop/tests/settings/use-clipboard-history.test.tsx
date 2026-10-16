// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import {
  useClipboardHistory,
  useClipboardHistoryCloud,
  type ClipboardHistoryClient,
  type ClipboardHistoryEntry,
  type CloudClipboardRequest,
} from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const entry = { text: "合成记录", timestampMs: 1, pinned: false };
const options = {
  historyEnabled: true,
  persistedHistoryEnabled: true,
  ios: false,
  page: "tools",
  onToggle: vi.fn(),
  onError: vi.fn(),
};

test("公共历史状态独立提供两步清空、固定和复制", async () => {
  const client = {
    clear: vi.fn(async () => {}),
    list: vi.fn(async () => [entry]),
    copy: vi.fn(async (_text: string) => {}),
    setPinned: vi.fn(async (_text: string, _pinned: boolean) => {}),
  };
  const hook = renderHook(() => useClipboardHistory({ ...options, client }));
  await waitFor(() => expect(hook.result.current.entries).toEqual([entry]));
  await act(async () => hook.result.current.setPinned(entry.text, true));
  expect(client.setPinned).toHaveBeenCalledWith(entry.text, true);
  await act(async () => hook.result.current.copy(entry.text));
  expect(client.copy).toHaveBeenCalledWith(entry.text);
  act(() => {
    void hook.result.current.clear();
  });
  expect(hook.result.current.clearArmed).toBe(true);
  expect(client.clear).not.toHaveBeenCalled();
  await act(async () => hook.result.current.clear());
  expect(client.clear).toHaveBeenCalledOnce();
  expect(hook.result.current.clearArmed).toBe(false);
});

test("历史写入同步互斥并在离页后刷新，较早读取不能覆盖", async () => {
  const removal = deferred<void>();
  const initialRead = deferred<ClipboardHistoryEntry[]>();
  const client = {
    clear: vi.fn(async () => {}),
    list: vi.fn().mockReturnValueOnce(initialRead.promise).mockResolvedValue([]),
    remove: vi.fn(() => removal.promise),
  };
  const hook = renderHook(({ page }) => useClipboardHistory({ ...options, client, page }), {
    initialProps: { page: "tools" },
  });
  let first: Promise<void> | undefined;
  act(() => {
    first = hook.result.current.remove(entry.text);
    void hook.result.current.remove(entry.text);
  });
  expect(client.remove).toHaveBeenCalledOnce();
  hook.rerender({ page: "about" });
  expect(hook.result.current.historyBusy).toBe(true);
  await act(async () => {
    removal.resolve();
    await first;
  });
  expect(client.list).toHaveBeenCalledTimes(2);
  await act(async () => initialRead.resolve([entry]));
  expect(hook.result.current.entries).toEqual([]);
  expect(hook.result.current.historyBusy).toBe(false);
});

test("自动同步只跟随保存开关 false 到 true，回调和页面变化不重复", async () => {
  const sync = deferred<ClipboardHistoryEntry[]>();
  const client = {
    clear: vi.fn(async () => {}),
    list: vi.fn(async () => []),
    sync: vi.fn(() => sync.promise),
  };
  const hook = renderHook(
    ({ saved, page, onError }) =>
      useClipboardHistory({ ...options, client, persistedHistoryEnabled: saved, page, onError }),
    { initialProps: { saved: false, page: "tools", onError: vi.fn() } },
  );
  expect(client.sync).not.toHaveBeenCalled();
  hook.rerender({ saved: true, page: "tools", onError: vi.fn() });
  expect(client.sync).toHaveBeenCalledOnce();
  hook.rerender({ saved: true, page: "about", onError: vi.fn() });
  await act(async () => sync.resolve([entry]));
  expect(hook.result.current.entries).toEqual([entry]);
  expect(client.sync).toHaveBeenCalledOnce();
});

test("客户端切换清除确认，旧错误和收尾不影响新操作", async () => {
  const oldRemove = deferred<void>();
  const nextRemove = deferred<void>();
  const make = (remove: () => Promise<void>): ClipboardHistoryClient => ({
    clear: vi.fn(async () => {}),
    list: async () => [entry],
    remove,
  });
  const first = make(() => oldRemove.promise);
  const next = make(() => nextRemove.promise);
  const onError = vi.fn();
  const hook = renderHook(({ client }) => useClipboardHistory({ ...options, client, onError }), {
    initialProps: { client: first },
  });
  await waitFor(() => expect(hook.result.current.entries).toEqual([entry]));
  act(() => {
    void hook.result.current.clear();
    void hook.result.current.remove(entry.text);
  });
  hook.rerender({ client: next });
  expect(hook.result.current.clearArmed).toBe(false);
  act(() => {
    void hook.result.current.remove(entry.text);
  });
  await act(async () => oldRemove.reject(new Error("合成旧失败")));
  expect(onError).not.toHaveBeenCalled();
  expect(hook.result.current.historyBusy).toBe(true);
  await act(async () => nextRemove.resolve());
  expect(hook.result.current.historyBusy).toBe(false);
});

test("云端公共状态同步互斥，换路由隔离旧请求", async () => {
  const sent = deferred<{ items: [] }>();
  const cloudRequest = vi.fn<CloudClipboardRequest>(async (action) =>
    action.operation === "add" ? sent.promise : { items: [] },
  );
  const hook = renderHook(
    ({ cloudRoute }) => useClipboardHistoryCloud({ cloudRequest, cloudRoute, historyShown: true }),
    { initialProps: { cloudRoute: "synthetic-a" } },
  );
  await waitFor(() => expect(hook.result.current.cloud).toBe("ready"));
  act(() => {
    void hook.result.current.sendToCloud(entry.text);
    void hook.result.current.sendToCloud(entry.text);
  });
  expect(cloudRequest.mock.calls.filter(([action]) => action.operation === "add")).toHaveLength(1);
  hook.rerender({ cloudRoute: "synthetic-b" });
  await waitFor(() => expect(hook.result.current.cloud).toBe("ready"));
  await act(async () => sent.resolve({ items: [] }));
  expect(hook.result.current.cloudNotice).toBe("");
  expect(hook.result.current.sending).toBe(false);
});

test("云端拒绝超长记录，不截断或发出请求", async () => {
  const cloudRequest = vi.fn<CloudClipboardRequest>(async () => ({ items: [] }));
  const hook = renderHook(() => useClipboardHistoryCloud({ cloudRequest, historyShown: true }));
  await waitFor(() => expect(hook.result.current.cloud).toBe("ready"));
  await act(async () => hook.result.current.sendToCloud("合".repeat(4001)));
  expect(cloudRequest).toHaveBeenCalledTimes(1);
  expect(hook.result.current.cloudNotice).toContain("4000 个 UTF-16 单元");
});
