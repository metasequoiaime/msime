// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useTemporaryFlag } from "../../../../packages/ui/src/core/use-temporary-flag";

beforeEach(() => vi.useFakeTimers());

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("a temporary flag restarts its deadline and clears its timer on unmount", () => {
  const { result, unmount } = renderHook(() => useTemporaryFlag(1600));
  expect(result.current[0]).toBe(false);

  act(() => result.current[1]());
  expect(result.current[0]).toBe(true);
  act(() => vi.advanceTimersByTime(1000));
  act(() => result.current[1]());
  act(() => vi.advanceTimersByTime(1000));
  expect(result.current[0]).toBe(true);
  act(() => vi.advanceTimersByTime(600));
  expect(result.current[0]).toBe(false);

  act(() => result.current[1]());
  const showAfterUnmount = result.current[1];
  unmount();
  expect(vi.getTimerCount()).toBe(0);
  act(() => showAfterUnmount());
  expect(vi.getTimerCount()).toBe(0);
});
