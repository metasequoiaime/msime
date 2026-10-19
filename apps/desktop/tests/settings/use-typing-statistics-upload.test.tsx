// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useTypingStatisticsUpload,
  type TypingStatisticsUploadClient,
  type TypingStatisticsUploadStatus,
} from "@msime/ui";

afterEach(cleanup);

const off: TypingStatisticsUploadStatus = { enabled: false, signedIn: true };
const on: TypingStatisticsUploadStatus = { enabled: true, signedIn: true, lastUploadMs: 1 };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function client() {
  return {
    sync: vi.fn(async () => off),
    status: vi.fn(async () => off),
    setEnabled: vi.fn(async (enabled: boolean) => (enabled ? on : off)),
  };
}

test("自动补传只随客户端启动，回包通知最新回调", async () => {
  const pending = deferred<TypingStatisticsUploadStatus>();
  const upload = client();
  upload.sync.mockReturnValueOnce(pending.promise);
  const previous = vi.fn();
  const next = vi.fn();
  const hook = renderHook(
    ({ onEnabledChange }) => useTypingStatisticsUpload({ client: upload, onEnabledChange }),
    { initialProps: { onEnabledChange: previous } },
  );
  expect(hook.result.current.status).toBeUndefined();
  hook.rerender({ onEnabledChange: next });
  await act(async () => pending.resolve(on));
  expect(upload.sync).toHaveBeenCalledOnce();
  expect(upload.status).not.toHaveBeenCalled();
  expect(hook.result.current.status).toEqual(on);
  expect(next).toHaveBeenCalledExactlyOnceWith(true);
  expect(previous).not.toHaveBeenCalled();
});

test("自动补传失败后静默读取开关，补读也失败时保持未知状态", async () => {
  const upload = client();
  upload.sync.mockRejectedValueOnce({ code: "account_unavailable" });
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload }));
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  expect(upload.status).toHaveBeenCalledOnce();
  expect(hook.result.current.message).toBe("");
  hook.unmount();

  upload.sync.mockRejectedValueOnce({ code: "account_unavailable" });
  upload.status.mockRejectedValueOnce({ code: "storage" });
  const failed = renderHook(() => useTypingStatisticsUpload({ client: upload }));
  await act(async () => {});
  expect(upload.status).toHaveBeenCalledTimes(2);
  expect(failed.result.current.status).toBeUndefined();
  expect(failed.result.current.busy).toBe(false);
  expect(failed.result.current.message).toBe("");
});

test("开关同步防重并公布宿主返回状态，完成后允许关闭", async () => {
  const upload = client();
  const pending = deferred<TypingStatisticsUploadStatus>();
  upload.setEnabled.mockReturnValueOnce(pending.promise);
  const onEnabledChange = vi.fn();
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload, onEnabledChange }));
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  act(() => {
    void hook.result.current.toggle(true);
    void hook.result.current.toggle(false);
  });
  expect(upload.setEnabled).toHaveBeenCalledExactlyOnceWith(true);
  expect(hook.result.current.busy).toBe(true);
  await act(async () => pending.resolve(on));
  expect(hook.result.current.status).toEqual(on);
  expect(hook.result.current.message).toBe("已打开上传，最近 7 天的每日计数已传到你的水杉账号。");
  expect(hook.result.current.busy).toBe(false);
  expect(onEnabledChange).toHaveBeenLastCalledWith(true);
  await act(async () => hook.result.current.toggle(false));
  expect(hook.result.current.status).toEqual(off);
  expect(hook.result.current.message).toBe("已关闭上传，云端的打字统计已删除。");
});

test.each([
  ["account_unauthorized", true, "先在「账号」页登录水杉账号，再打开上传。"],
  ["account_rate_limited", true, "上传过于频繁，过一会儿会自动再传。"],
  ["storage", true, "无法读取或保存上传设置，请稍后重试。"],
  ["account_unavailable", true, "已打开上传，但这次没有传上去，稍后会自动再试。"],
  ["account_unavailable", false, "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。"],
] as const)("%s 失败时保留%s动作的说明并补读实际状态", async (code, enabling, message) => {
  const upload = client();
  upload.setEnabled.mockRejectedValueOnce({ code });
  upload.status.mockResolvedValueOnce(enabling ? on : off);
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload }));
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  await act(async () => hook.result.current.toggle(enabling));
  expect(hook.result.current.message).toBe(message);
  expect(hook.result.current.status).toEqual(enabling ? on : off);
  expect(upload.status).toHaveBeenCalledOnce();
  expect(hook.result.current.busy).toBe(false);
});

test("手动变更后的状态不被更早的自动补传回包覆盖", async () => {
  const pending = deferred<TypingStatisticsUploadStatus>();
  const upload = client();
  upload.sync.mockReturnValueOnce(pending.promise);
  const onEnabledChange = vi.fn();
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload, onEnabledChange }));
  await act(async () => hook.result.current.toggle(true));
  await act(async () => pending.resolve(off));
  expect(hook.result.current.status).toEqual(on);
  expect(onEnabledChange).toHaveBeenCalledExactlyOnceWith(true);
});

test("手动操作的补读失败不覆盖原错误，下一次变更仍可进行", async () => {
  const upload = client();
  upload.setEnabled.mockRejectedValueOnce({ code: "storage" });
  upload.status.mockRejectedValueOnce({ code: "account_unavailable" });
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload }));
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  await act(async () => hook.result.current.toggle(true));
  expect(hook.result.current.message).toBe("无法读取或保存上传设置，请稍后重试。");
  expect(hook.result.current.busy).toBe(false);
  await act(async () => hook.result.current.toggle(true));
  expect(hook.result.current.status).toEqual(on);
});

test("自动补传的失败补读在客户端换走又换回后仍然过期", async () => {
  const recovery = deferred<TypingStatisticsUploadStatus>();
  const previous = client();
  previous.sync.mockRejectedValueOnce({ code: "storage" });
  previous.status.mockReturnValueOnce(recovery.promise);
  const next = client();
  const onEnabledChange = vi.fn();
  const hook = renderHook(({ client }) => useTypingStatisticsUpload({ client, onEnabledChange }), {
    initialProps: { client: previous as TypingStatisticsUploadClient },
  });
  await waitFor(() => expect(previous.status).toHaveBeenCalledOnce());
  hook.rerender({ client: next });
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  hook.rerender({ client: previous });
  await waitFor(() => expect(previous.sync).toHaveBeenCalledTimes(2));
  onEnabledChange.mockClear();
  await act(async () => recovery.resolve(on));
  expect(hook.result.current.status).toEqual(off);
  expect(onEnabledChange).not.toHaveBeenCalled();
});

test("自动补传在客户端离开后失败，不再启动旧客户端补读", async () => {
  const pending = deferred<TypingStatisticsUploadStatus>();
  const previous = client();
  previous.sync.mockReturnValueOnce(pending.promise);
  const next = client();
  const hook = renderHook(({ client }) => useTypingStatisticsUpload({ client }), {
    initialProps: { client: previous as TypingStatisticsUploadClient },
  });
  hook.rerender({ client: next });
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  await act(async () => pending.reject({ code: "account_unavailable" }));
  expect(previous.status).not.toHaveBeenCalled();
});

test("客户端变化立即清掉旧状态和错误，并且旧结果不释放新操作的忙碌锁", async () => {
  const previous = client();
  const recovery = deferred<TypingStatisticsUploadStatus>();
  previous.setEnabled.mockRejectedValueOnce({ code: "storage" });
  previous.status.mockReturnValueOnce(recovery.promise);
  const next = client();
  const loading = deferred<TypingStatisticsUploadStatus>();
  const mutating = deferred<TypingStatisticsUploadStatus>();
  next.sync.mockReturnValueOnce(loading.promise);
  next.setEnabled.mockReturnValueOnce(mutating.promise);
  const notify = vi.fn();
  const hook = renderHook(
    ({ client }) => useTypingStatisticsUpload({ client, onEnabledChange: notify }),
    {
      initialProps: { client: previous as TypingStatisticsUploadClient },
    },
  );
  await waitFor(() => expect(hook.result.current.status).toEqual(off));
  act(() => void hook.result.current.toggle(true));
  await waitFor(() => expect(previous.status).toHaveBeenCalledOnce());
  expect(hook.result.current.message).not.toBe("");
  hook.rerender({ client: next });
  expect(hook.result.current.status).toBeUndefined();
  expect(hook.result.current.message).toBe("");
  expect(hook.result.current.busy).toBe(false);
  await act(async () => loading.resolve(off));
  act(() => void hook.result.current.toggle(true));
  notify.mockClear();
  await act(async () => recovery.resolve(on));
  expect(notify).not.toHaveBeenCalled();
  expect(hook.result.current.busy).toBe(true);
  await act(async () => mutating.resolve(on));
  expect(hook.result.current.busy).toBe(false);
  expect(notify).toHaveBeenCalledExactlyOnceWith(true);
});

test("StrictMode 重新挂载后只采用当前补传，卸载后不启动补读", async () => {
  const first = deferred<TypingStatisticsUploadStatus>();
  const second = deferred<TypingStatisticsUploadStatus>();
  const upload = client();
  upload.sync.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const onEnabledChange = vi.fn();
  const hook = renderHook(() => useTypingStatisticsUpload({ client: upload, onEnabledChange }), {
    wrapper: StrictMode,
  });
  await act(async () => first.resolve(on));
  expect(onEnabledChange).not.toHaveBeenCalled();
  hook.unmount();
  await act(async () => second.reject({ code: "account_unavailable" }));
  expect(upload.status).not.toHaveBeenCalled();
  expect(onEnabledChange).not.toHaveBeenCalled();
});
