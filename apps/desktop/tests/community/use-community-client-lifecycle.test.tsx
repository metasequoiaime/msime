// @vitest-environment jsdom
import { expect, test } from "vitest";
import { renderHook } from "@testing-library/react";
import { useCommunityClientLifecycle } from "@msime/ui";

test("invalidates stale actions and clears the running lock when the client changes", () => {
  const firstClient = {};
  const secondClient = {};
  const { result, rerender } = renderHook(({ client }) => useCommunityClientLifecycle(client), {
    initialProps: { client: firstClient },
  });

  const firstGeneration = result.current.clientGeneration.current;
  result.current.actionRunning.current = true;

  rerender({ client: secondClient });

  const secondGeneration = result.current.clientGeneration.current;
  expect(secondGeneration).toBeGreaterThan(firstGeneration);
  expect(result.current.actionRunning.current).toBe(false);
  expect(result.current.isCurrent(firstGeneration)).toBe(false);
  expect(result.current.isCurrent(secondGeneration)).toBe(true);
});

test("marks actions stale after the owning component unmounts", () => {
  const { result, unmount } = renderHook(() => useCommunityClientLifecycle({}));
  const generation = result.current.clientGeneration.current;

  unmount();

  expect(result.current.isCurrent(generation)).toBe(false);
});
