// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DictionaryUpdates } from "../../../../packages/ui/src/settings/dictionary-updates";
import type { DictionaryUpdateStatus } from "../../../../packages/ui/src/dictionary/dictionary-contracts";

const ready: DictionaryUpdateStatus = {
  enabled: true,
  version: "dict-v2.0.30",
  phase: "ready",
  checked_at: 1,
  downloaded_bytes: 1024,
  total_bytes: 1024,
  error: null,
  can_rollback: true,
  pending_restart: true,
};
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("展示待生效版本，开关和回退传递到共享宿主", async () => {
  const request = vi.fn(async () => ready);
  render(<DictionaryUpdates request={request} />);
  expect(await screen.findByText("dict-v2.0.30")).toBeTruthy();
  expect(screen.getByText(/下次启动输入法/)).toBeTruthy();
  fireEvent.click(screen.getByRole("switch"));
  expect(request).toHaveBeenCalledWith("configure", false);
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(request).toHaveBeenCalledWith("check", undefined);
  fireEvent.click(screen.getByRole("button", { name: "恢复" }));
  expect(request).toHaveBeenCalledWith("rollback", undefined);
});

test("下载期间禁止重复检查，失败保留旧词库提示", async () => {
  const request = vi.fn(async () => ({ ...ready, phase: "downloading" }));
  const view = render(<DictionaryUpdates request={request} />);
  expect(
    ((await screen.findByRole("button", { name: "更新中…" })) as HTMLButtonElement).disabled,
  ).toBe(true);
  view.rerender(
    <DictionaryUpdates
      request={async () => ({ ...ready, phase: "failed", error: "dictionary_update_integrity" })}
    />,
  );
  expect(await screen.findByText("更新失败，继续使用原词库")).toBeTruthy();
});

test("卸载停止轮询，迟到请求不会重新启动定时器", async () => {
  vi.useFakeTimers();
  const request = vi.fn(async () => ready);
  const { unmount } = render(<DictionaryUpdates request={request} />);
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    vi.advanceTimersByTime(2000);
  });
  expect(request).toHaveBeenCalledTimes(2);
  unmount();
  await act(async () => {
    vi.advanceTimersByTime(6000);
  });
  expect(request).toHaveBeenCalledTimes(2);
});

function deferredStatus() {
  let resolve!: (status: DictionaryUpdateStatus) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<DictionaryUpdateStatus>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("普通重渲染保留两秒轮询边界，下载状态更新进度与操作可用性", async () => {
  vi.useFakeTimers();
  const request = vi
    .fn()
    .mockResolvedValueOnce(ready)
    .mockResolvedValueOnce({
      ...ready,
      phase: "downloading",
      downloaded_bytes: 1572864,
      total_bytes: 3145729,
    });
  const view = render(<DictionaryUpdates request={request} />);
  await act(async () => {});
  await act(async () => vi.advanceTimersByTime(1000));
  view.rerender(<DictionaryUpdates request={request} />);
  await act(async () => vi.advanceTimersByTime(999));
  expect(request).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTime(1));
  expect(request.mock.calls).toEqual([
    ["status", undefined],
    ["status", undefined],
  ]);
  expect(screen.getByText("正在下载 1 / 4 MiB")).toBeTruthy();
  expect((screen.getByRole("button", { name: "更新中…" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect((screen.getByRole("button", { name: "恢复" }) as HTMLButtonElement).disabled).toBe(true);
});

test.each(["success", "failure"])("旧轮询的迟到结果不覆盖最新状态（%s）", async (outcome) => {
  vi.useFakeTimers();
  const old = deferredStatus();
  const request = vi
    .fn()
    .mockReturnValueOnce(old.promise)
    .mockResolvedValue({ ...ready, version: "synthetic-latest" });
  render(<DictionaryUpdates request={request} />);
  await act(async () => vi.advanceTimersByTime(2000));
  expect(screen.getByText("synthetic-latest")).toBeTruthy();
  await act(async () => {
    if (outcome === "success") old.resolve({ ...ready, version: "synthetic-stale" });
    else old.reject(new Error("synthetic stale failure"));
  });
  expect(screen.getByText("synthetic-latest")).toBeTruthy();
  expect(screen.queryByText("synthetic-stale")).toBeNull();
  expect(screen.queryByText("词库更新暂不可用，请稍后重试")).toBeNull();
});

test("入口替换使旧请求失效，立即刷新并重新计算轮询边界", async () => {
  vi.useFakeTimers();
  const old = deferredStatus();
  const next = deferredStatus();
  const oldRequest = vi.fn().mockResolvedValueOnce(ready).mockReturnValueOnce(old.promise);
  const nextRequest = vi.fn(() => next.promise);
  const view = render(<DictionaryUpdates request={oldRequest} />);
  await act(async () => {});
  await act(async () => vi.advanceTimersByTime(3000));
  view.rerender(<DictionaryUpdates request={nextRequest} />);
  expect(nextRequest).toHaveBeenCalledWith("status", undefined);
  expect(screen.getByText(ready.version)).toBeTruthy();
  await act(async () => old.resolve({ ...ready, version: "synthetic-old-owner" }));
  expect(screen.queryByText("synthetic-old-owner")).toBeNull();
  await act(async () => vi.advanceTimersByTime(1000));
  expect(nextRequest).toHaveBeenCalledTimes(1);
  await act(async () => next.resolve({ ...ready, version: "synthetic-new-owner" }));
  expect(screen.getByText("synthetic-new-owner")).toBeTruthy();
  await act(async () => vi.advanceTimersByTime(1000));
  expect(nextRequest).toHaveBeenCalledTimes(2);
  expect(oldRequest).toHaveBeenCalledTimes(2);
});

test("当前请求失败保留版本，后续轮询成功清除错误", async () => {
  vi.useFakeTimers();
  const request = vi
    .fn()
    .mockResolvedValueOnce(ready)
    .mockRejectedValueOnce(new Error("synthetic offline"))
    .mockResolvedValueOnce({ ...ready, phase: "current", pending_restart: false });
  render(<DictionaryUpdates request={request} />);
  await act(async () => {});
  await act(async () => vi.advanceTimersByTime(2000));
  expect(screen.getByText(ready.version)).toBeTruthy();
  expect(screen.getByText("词库更新暂不可用，请稍后重试")).toBeTruthy();
  await act(async () => vi.advanceTimersByTime(2000));
  expect(screen.queryByText("词库更新暂不可用，请稍后重试")).toBeNull();
  expect(screen.getByText("词库已是最新版本")).toBeTruthy();
});
