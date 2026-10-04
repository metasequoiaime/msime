// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { StrictMode } from "react";
import { useAsyncGeneration } from "../../../../packages/ui/src/settings/use-async-generation";

afterEach(cleanup);

test("keeps active work valid when owners have not changed", () => {
  const owner = {};
  const { result, rerender } = renderHook(() => useAsyncGeneration(owner));
  const generation = result.current;
  const request = ++generation.current;

  rerender();

  expect(result.current).toBe(generation);
  expect(generation.current).toBe(request);
});

test("invalidates active work when any owner changes", () => {
  const { result, rerender } = renderHook(({ owner }) => useAsyncGeneration("fixture", owner), {
    initialProps: { owner: {} },
  });
  const generation = result.current;
  const request = ++generation.current;

  rerender({ owner: {} });

  expect(result.current).toBe(generation);
  expect(generation.current).toBeGreaterThan(request);
});

test.each([false, true])("invalidates manually advanced work on unmount (strict=%s)", (strict) => {
  const { result, unmount } = renderHook(() => useAsyncGeneration(), {
    wrapper: ({ children }) => (strict ? <StrictMode>{children}</StrictMode> : children),
  });
  const generation = result.current;
  const request = ++generation.current;

  unmount();

  expect(generation.current).toBeGreaterThan(request);
});

test("rejects a pending result after its owner unmounts", async () => {
  const { result, unmount } = renderHook(() => useAsyncGeneration());
  const generation = result.current;
  const request = ++generation.current;
  const applied: string[] = [];
  let resolve!: (value: string) => void;
  const pending = new Promise<string>((done) => {
    resolve = done;
  }).then((value) => {
    if (generation.current === request) applied.push(value);
  });

  unmount();
  await act(async () => {
    resolve("fixture late result");
    await pending;
  });

  expect(applied).toEqual([]);
});
