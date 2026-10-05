// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useAsyncActionRunner } from "@msime/ui";

test("serializes actions and ignores stale owner results", async () => {
  const owner = {};
  const setError = vi.fn();
  let resolve!: () => void;
  const { result, rerender } = renderHook(
    ({ currentOwner }) => useAsyncActionRunner(setError, undefined, currentOwner),
    { initialProps: { currentOwner: owner } },
  );

  let first!: Promise<void> | undefined;
  act(() => {
    first = result.current.run(() => new Promise<void>((finish) => (resolve = finish)), {
      formatError: () => "失败",
    });
  });
  expect(result.current.busy).toBe(true);
  expect(result.current.run(async () => {}, { formatError: () => "不应执行" })).toBeUndefined();

  rerender({ currentOwner: {} });
  resolve();
  await act(async () => {
    await first;
  });
  expect(result.current.busy).toBe(false);
  expect(setError).not.toHaveBeenCalledWith("失败");
});

test.each(["resolve", "reject"])(
  "a stale action that %s cannot unlock its replacement",
  async (completion) => {
    const setError = vi.fn();
    const { result, rerender } = renderHook(
      ({ owner }) => useAsyncActionRunner(setError, undefined, owner),
      { initialProps: { owner: {} } },
    );
    let finishFirst!: () => void;
    let finishSecond!: () => void;
    let first!: Promise<void> | undefined;
    let second!: Promise<void> | undefined;
    act(() => {
      first = result.current.run(
        () =>
          new Promise<void>((resolve, reject) => {
            finishFirst = () => (completion === "resolve" ? resolve() : reject(new Error("stale")));
          }),
        { formatError: () => "旧操作失败" },
      );
    });
    rerender({ owner: {} });
    act(() => {
      second = result.current.run(() => new Promise<void>((resolve) => (finishSecond = resolve)), {
        formatError: () => "新操作失败",
      });
    });
    await act(async () => {
      finishFirst();
      await first;
    });
    const runningAfterStaleCompletion = result.current.running.current;
    const thirdOperation = vi.fn(async () => {});
    let third: Promise<void> | undefined;
    act(() => {
      third = result.current.run(thirdOperation, { formatError: () => "不应执行" });
    });
    await act(async () => {
      finishSecond();
      await second;
      await third;
    });

    expect(runningAfterStaleCompletion).toBe(true);
    expect(third).toBeUndefined();
    expect(thirdOperation).not.toHaveBeenCalled();
    expect(setError).not.toHaveBeenCalledWith("旧操作失败");
    expect(result.current.running.current).toBe(false);
    expect(result.current.busy).toBe(false);
  },
);
