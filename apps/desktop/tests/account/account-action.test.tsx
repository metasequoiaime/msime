// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useAccountAction } from "../../../../packages/ui/src/account/account-operation";

test("serializes account actions and clears status before running", async () => {
  const setError = vi.fn();
  const setNotice = vi.fn();
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const client = {};
  const { result } = renderHook(() => useAccountAction(client, setError, setNotice));
  let first!: Promise<void>;
  act(() => {
    first = result.current.perform(() => pending)!;
  });

  expect(result.current.perform(async () => {})).toBeUndefined();
  expect(result.current.busy).toBe(true);
  expect(setError).toHaveBeenCalledWith("");
  expect(setNotice).toHaveBeenCalledWith("");

  resolve();
  await act(async () => {
    await first;
  });
  expect(result.current.busy).toBe(false);
});
