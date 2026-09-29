// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useWindowResizeCapture } from "@msime/ui";

test("resizes from an edge and reports host failures", async () => {
  const resizeWindow = vi.fn().mockRejectedValue(new Error("synthetic"));
  const setError = vi.fn();
  const { result } = renderHook(() =>
    useWindowResizeCapture({ resizeWindow, windowMaximized: false, setError }),
  );
  const preventDefault = vi.fn();
  const stopPropagation = vi.fn();
  const event = {
    button: 0,
    clientX: 799,
    clientY: 599,
    currentTarget: {
      getBoundingClientRect: () => ({ left: 0, top: 0, right: 800, bottom: 600 }),
    },
    preventDefault,
    stopPropagation,
  } as never;

  result.current(event);
  expect(resizeWindow).toHaveBeenCalledWith("se");
  expect(preventDefault).toHaveBeenCalledOnce();
  expect(stopPropagation).toHaveBeenCalledOnce();
  await vi.waitFor(() => expect(setError).toHaveBeenCalledWith("无法调整窗口大小，请重试。"));
});

test("ignores non-primary presses and maximized windows", () => {
  const resizeWindow = vi.fn().mockResolvedValue(undefined);
  const event = {
    button: 2,
    currentTarget: { getBoundingClientRect: () => ({ left: 0, top: 0, right: 800, bottom: 600 }) },
  } as never;
  const { result: maximized } = renderHook(() =>
    useWindowResizeCapture({ resizeWindow, windowMaximized: true, setError: vi.fn() }),
  );
  maximized.current(event);
  expect(resizeWindow).not.toHaveBeenCalled();
});
