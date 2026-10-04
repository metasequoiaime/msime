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
