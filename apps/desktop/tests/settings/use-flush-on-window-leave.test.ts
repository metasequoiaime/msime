// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useFlushOnWindowLeave } from "../../../../packages/ui/src/settings/use-flush-on-window-leave";

afterEach(() => {
  vi.restoreAllMocks();
});

test("flushes on blur, pagehide, and hidden visibility changes", () => {
  const flush = vi.fn();
  renderHook(() => useFlushOnWindowLeave(flush));

  act(() => window.dispatchEvent(new Event("blur")));
  act(() => window.dispatchEvent(new Event("pagehide")));
  act(() => {
    vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    document.dispatchEvent(new Event("visibilitychange"));
  });

  expect(flush).toHaveBeenCalledTimes(3);
});

test("uses the latest flush callback without accumulating listeners", () => {
  const first = vi.fn();
  const second = vi.fn();
  const { rerender } = renderHook(({ flush }) => useFlushOnWindowLeave(flush), {
    initialProps: { flush: first },
  });

  rerender({ flush: second });
  act(() => window.dispatchEvent(new Event("blur")));

  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledOnce();
});

test("the settings autosave hook uses the shared window leave lifecycle", () => {
  const persistence = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-settings-persistence.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(persistence).toContain('from "./use-flush-on-window-leave"');
  expect(persistence).toContain("useFlushOnWindowLeave");
  expect(persistence).not.toContain('window.addEventListener("blur"');
});
