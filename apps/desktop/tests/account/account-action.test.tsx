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

test("invalidates account actions when an owner changes", () => {
  const setError = vi.fn();
  const setNotice = vi.fn();
  const client = {};
  const { result, rerender } = renderHook(
    ({ owner }) => useAccountAction(client, setError, setNotice, owner),
    { initialProps: { owner: "first-user" } },
  );
  const firstGeneration = result.current.clientGeneration.current;

  rerender({ owner: "second-user" });

  expect(result.current.clientGeneration.current).toBeGreaterThan(firstGeneration);
});

test("allows owner-reset actions to bypass the captured busy state", async () => {
  const setError = vi.fn();
  const setNotice = vi.fn();
  const client = {};
  const blockedOperation = vi.fn(async () => {});
  const allowedOperation = vi.fn(async () => {});
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const { result, rerender } = renderHook(
    ({ owner }) => useAccountAction(client, setError, setNotice, owner),
    { initialProps: { owner: "first" } },
  );

  let first!: Promise<void>;
  act(() => {
    first = result.current.perform(() => pending)!;
  });
  const performWhileBusy = result.current.perform;
  rerender({ owner: "second" });

  expect(performWhileBusy(blockedOperation)).toBeUndefined();
  const replacement = performWhileBusy(allowedOperation, { allowBusy: true });
  expect(replacement).toBeDefined();

  await act(async () => {
    release();
    await first;
    await replacement;
  });
  expect(blockedOperation).not.toHaveBeenCalled();
  expect(allowedOperation).toHaveBeenCalledOnce();
});
