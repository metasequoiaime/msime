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
