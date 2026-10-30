// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import {
  useCloudClipboardPanel,
  type CloudClipboardAction,
  type CloudClipboardPanelClient,
} from "@msime/ui";

afterEach(cleanup);

type CloudClipboardResponse = Awaited<ReturnType<CloudClipboardPanelClient["request"]>>;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function page(text = "合成条目", pendingDevices = 0): CloudClipboardResponse {
  return {
    enabled: true,
    retention_days: 0,
    items: [{ id: "synthetic-item", text }],
    e2e: {
      status: "ready",
      code: "1111 2222 3333 4444",
      pending_devices: pendingDevices,
      undecryptable: 0,
    },
  };
}

const otherDevice = {
  device_id: "synthetic-other-device",
  name: "合成设备",
  created_at: "2026-10-19T00:00:00Z",
  approved: false,
  current: false,
  code: "5555 6666 7777 8888",
};

function devices(): CloudClipboardResponse {
  return {
    enabled: true,
    status: "ready",
    device_id: "synthetic-current-device",
    code: "1111 2222 3333 4444",
    devices: [otherDevice],
  };
}

test("更换宿主隔离迟到列表、能力与输入目标错误，保留原草稿和搜索", async () => {
  const oldList = deferred<CloudClipboardResponse>();
  const oldCapability = deferred<boolean>();
  const oldTarget = deferred<void>();
  const previous: CloudClipboardPanelClient = {
    close: vi.fn(),
    request: () => oldList.promise,
    canSendText: () => oldCapability.promise,
    rememberInputTarget: () => oldTarget.promise,
    sendText: vi.fn(),
  };
  const hook = renderHook(({ client }) => useCloudClipboardPanel(client), {
    initialProps: { client: previous },
  });
  act(() => {
    hook.result.current.updateDraft("合成草稿");
    hook.result.current.setSearch("合成搜索");
  });
  const next: CloudClipboardPanelClient = {
    close: vi.fn(),
    request: vi.fn().mockResolvedValue(page("合成新条目")),
    canSendText: vi.fn().mockResolvedValue(false),
    sendText: vi.fn().mockResolvedValue(undefined),
    copyText: vi.fn().mockResolvedValue(undefined),
  };
  hook.rerender({ client: next });
  await waitFor(() => expect(hook.result.current.loaded).toBe(true));
  await act(async () => {
    oldList.resolve(page("合成旧条目"));
    oldCapability.resolve(true);
    oldTarget.reject(new Error("synthetic"));
  });
  expect(hook.result.current.items).toEqual([{ id: "synthetic-item", text: "合成新条目" }]);
  expect(hook.result.current.notice).toBe("云剪贴板已刷新");
  expect(hook.result.current.draft).toBe("合成草稿");
  expect(hook.result.current.search).toBe("合成搜索");
  expect(next.request).toHaveBeenCalledWith({ operation: "list", search: "合成搜索" });
  await act(async () => hook.result.current.choose(hook.result.current.items[0]));
  expect(next.copyText).toHaveBeenCalledWith("合成新条目");
  expect(next.sendText).not.toHaveBeenCalled();
});

test.each([
  { kind: "disable" as const },
  { kind: "reset" as const },
  { kind: "remove" as const, device: otherDevice },
])("更换宿主清除旧的 $kind 确认，不得把旧操作带到新客户端", async (confirmation) => {
  const previous: CloudClipboardPanelClient = {
    close: vi.fn(),
    request: vi.fn().mockResolvedValue(page("合成旧条目")),
  };
  const nextRequest = vi.fn().mockResolvedValue(page("合成新条目"));
  const next: CloudClipboardPanelClient = { close: vi.fn(), request: nextRequest };
  const hook = renderHook(({ client }) => useCloudClipboardPanel(client), {
    initialProps: { client: previous },
  });
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  act(() => hook.result.current.setConfirmation(confirmation));
  expect(hook.result.current.confirmation).toEqual(confirmation);
  hook.rerender({ client: next });
  await waitFor(() => expect(hook.result.current.loaded).toBe(true));
  expect(hook.result.current.confirmation).toBeNull();
  expect(nextRequest).not.toHaveBeenCalledWith({ operation: "set_enabled", enabled: false });
  expect(nextRequest).not.toHaveBeenCalledWith({ operation: "reset" });
  expect(nextRequest).not.toHaveBeenCalledWith({
    operation: "remove",
    device_id: otherDevice.device_id,
  });
});

test("StrictMode 重放后只采纳当前列表，旧完成不解除当前忙状态", async () => {
  const pending: ReturnType<typeof deferred<CloudClipboardResponse>>[] = [];
  const client: CloudClipboardPanelClient = {
    close: vi.fn(),
    request: () => {
      const value = deferred<CloudClipboardResponse>();
      pending.push(value);
      return value.promise;
    },
  };
  const hook = renderHook(() => useCloudClipboardPanel(client), { reactStrictMode: true });
  expect(pending).toHaveLength(2);
  await act(async () => pending[0].resolve(page("合成旧条目")));
  expect(hook.result.current.loaded).toBe(false);
  expect(hook.result.current.busy).toBe(true);
  await act(async () => pending[1].resolve(page("合成当前条目")));
  expect(hook.result.current.items[0].text).toBe("合成当前条目");
  expect(hook.result.current.busy).toBe(false);
});

test("上传同步防重，完成保留新草稿，并按最新搜索刷新", async () => {
  const uploaded = deferred<CloudClipboardResponse>();
  const request = vi.fn((action: CloudClipboardAction) =>
    action.operation === "add" ? uploaded.promise : Promise.resolve(page()),
  );
  const client: CloudClipboardPanelClient = { close: vi.fn(), request };
  const hook = renderHook(() => useCloudClipboardPanel(client));
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  act(() => hook.result.current.updateDraft("合成上传"));
  let saving!: Promise<void> | undefined;
  act(() => {
    saving = hook.result.current.add();
    void hook.result.current.add();
  });
  expect(request.mock.calls.filter(([action]) => action.operation === "add")).toHaveLength(1);
  act(() => {
    hook.result.current.updateDraft("合成新草稿");
    hook.result.current.setSearch("合成新搜索");
  });
  await act(async () => {
    uploaded.resolve({});
    await saving;
  });
  expect(hook.result.current.draft).toBe("合成新草稿");
  expect(hook.result.current.notice).toBe("已上传");
  expect(request).toHaveBeenLastCalledWith({ operation: "list", search: "合成新搜索" });
  await act(async () => hook.result.current.add());
  expect(hook.result.current.draft).toBe("");
});

test("等待设备派生值与确认文案保持，关闭、重置和移除均通过确认动作执行", async () => {
  const request = vi.fn(async (action: CloudClipboardAction) => {
    if (action.operation === "list") return page("合成条目", 1);
    if (action.operation === "set_enabled") return { enabled: action.enabled };
    return devices();
  });
  const client: CloudClipboardPanelClient = { close: vi.fn(), request };
  const hook = renderHook(() => useCloudClipboardPanel(client));
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  expect(hook.result.current.waiting).toEqual([otherDevice]);
  expect(hook.result.current.waitingCount).toBe(1);
  act(() => hook.result.current.toggle());
  expect(hook.result.current.confirmText?.action).toBe("确认关闭并删除历史");
  expect(request).not.toHaveBeenCalledWith({ operation: "set_enabled", enabled: false });
  await act(async () => hook.result.current.confirm());
  expect(hook.result.current.enabled).toBe(false);
  expect(hook.result.current.items).toEqual([]);
  expect(hook.result.current.devices).toBeNull();
  await act(async () => hook.result.current.toggle());
  expect(hook.result.current.ready).toBe(true);
  act(() => hook.result.current.setConfirmation({ kind: "reset" }));
  expect(hook.result.current.confirmText?.message).toContain("其他所有设备");
  await act(async () => hook.result.current.confirm());
  expect(request).toHaveBeenCalledWith({ operation: "reset" });
  act(() => hook.result.current.setConfirmation({ kind: "remove", device: otherDevice }));
  expect(hook.result.current.confirmText?.label).toBe("移除设备");
  await act(async () => hook.result.current.confirm());
  expect(request).toHaveBeenCalledWith({ operation: "remove", device_id: otherDevice.device_id });
});

test("刷新失败清空旧条目和设备，移除确认也随失败清理", async () => {
  let unavailable = false;
  const request = vi.fn(async (action: CloudClipboardAction) => {
    if (unavailable) throw new Error("synthetic");
    return action.operation === "list" ? page("合成条目", 1) : devices();
  });
  const client: CloudClipboardPanelClient = { close: vi.fn(), request };
  const hook = renderHook(() => useCloudClipboardPanel(client));
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  act(() => hook.result.current.setConfirmation({ kind: "remove", device: otherDevice }));
  unavailable = true;
  await act(async () => hook.result.current.refresh());
  expect(hook.result.current.items).toEqual([]);
  expect(hook.result.current.loaded).toBe(false);
  expect(hook.result.current.e2e).toBeNull();
  expect(hook.result.current.devices).toBeNull();
  expect(hook.result.current.confirmation).toBeNull();
  expect(hook.result.current.notice).toBe("无法访问云剪贴板服务");
});
