// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { focusAndRevealNearest } from "../../../../packages/ui/src/core/focus-and-reveal-nearest";

afterEach(() => {
  vi.restoreAllMocks();
});

test("focuses the target without moving first, then reveals its nearest visible position", () => {
  const target = document.createElement("button");
  const focus = vi.spyOn(target, "focus");
  const scrollIntoView = vi.fn();
  Object.defineProperty(target, "scrollIntoView", { configurable: true, value: scrollIntoView });

  focusAndRevealNearest(target);

  expect(focus).toHaveBeenCalledWith({ preventScroll: true });
  expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest", inline: "nearest" });
});

test("does nothing when there is no target", () => {
  expect(() => focusAndRevealNearest(null)).not.toThrow();
});
