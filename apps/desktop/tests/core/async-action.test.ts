import { expect, test, vi } from "vitest";
import { runAsyncAction } from "@msime/ui";

test("runs a current action and clears its optional notice", async () => {
  const setBusy = vi.fn();
  const setError = vi.fn();
  const setNotice = vi.fn();
  const operation = vi.fn(async (isCurrent: () => boolean) => {
    expect(isCurrent()).toBe(true);
  });

  await runAsyncAction(
    { busy: false, isCurrent: () => true, setBusy, setError, setNotice },
    operation,
    { formatError: () => "失败" },
  );

  expect(operation).toHaveBeenCalledOnce();
  expect(setBusy.mock.calls).toEqual([[true], [false]]);
  expect(setError).toHaveBeenCalledExactlyOnceWith("");
  expect(setNotice).toHaveBeenCalledExactlyOnceWith("");
});

test("maps failures only while current and can ignore cancellation errors", async () => {
  const make = () => ({
    setBusy: vi.fn(),
    setError: vi.fn(),
    setNotice: vi.fn(),
  });
  const ordinary = make();
  await runAsyncAction(
    { busy: false, isCurrent: () => true, ...ordinary },
    async () => {
      throw new Error("synthetic failure");
    },
    { formatError: () => "社区暂时不可用，请稍后重试。" },
  );
  expect(ordinary.setError).toHaveBeenCalledWith("社区暂时不可用，请稍后重试。");

  const cancelled = make();
  await runAsyncAction(
    { busy: false, isCurrent: () => true, ...cancelled },
    async () => {
      throw { code: "community_cancelled" };
    },
    {
      formatError: () => "不应显示",
      ignoreError: (error) => (error as { code?: string }).code === "community_cancelled",
    },
  );
  expect(cancelled.setError).not.toHaveBeenCalledWith("不应显示");

  const stale = make();
  let current = true;
  let reject!: (reason: unknown) => void;
  const pending = runAsyncAction(
    { busy: false, isCurrent: () => current, ...stale },
    () =>
      new Promise<void>((_resolve, fail) => {
        reject = fail;
      }),
    { formatError: () => "不应显示" },
  );
  current = false;
  reject(new Error("late"));
  await pending;
  expect(stale.setBusy.mock.calls).toEqual([[true]]);
  expect(stale.setError).toHaveBeenCalledExactlyOnceWith("");
});
