// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useDebouncedCallback } from "../../../../packages/ui/src/core/use-debounced-callback";

afterEach(() => {
  vi.useRealTimers();
});

test("只执行最后一次调用并可手动取消", () => {
  vi.useFakeTimers();
  const callback = vi.fn();
  const { result } = renderHook(() => useDebouncedCallback(callback, 1000));

  act(() => {
    result.current.schedule("first");
    result.current.schedule("last");
    vi.advanceTimersByTime(999);
  });
  expect(callback).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(1));
  expect(callback).toHaveBeenCalledOnce();
  expect(callback).toHaveBeenCalledWith("last");

  act(() => {
    result.current.schedule("cancelled");
    result.current.cancel();
    vi.advanceTimersByTime(1000);
  });
  expect(callback).toHaveBeenCalledOnce();
});

test("使用最新回调且卸载后不再执行", () => {
  vi.useFakeTimers();
  const first = vi.fn();
  const second = vi.fn();
  const { result, rerender, unmount } = renderHook(
    ({ callback }) => useDebouncedCallback(callback, 1000),
    {
      initialProps: { callback: first },
    },
  );

  act(() => result.current.schedule("value"));
  rerender({ callback: second });
  act(() => vi.advanceTimersByTime(1000));
  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledWith("value");

  act(() => result.current.schedule("unmounted"));
  unmount();
  act(() => vi.advanceTimersByTime(1000));
  expect(second).toHaveBeenCalledOnce();
});

test("延迟变化会取消旧计时器", () => {
  vi.useFakeTimers();
  const callback = vi.fn();
  const { result, rerender } = renderHook(({ delay }) => useDebouncedCallback(callback, delay), {
    initialProps: { delay: 1000 },
  });

  act(() => result.current.schedule());
  rerender({ delay: 2000 });
  act(() => vi.advanceTimersByTime(1000));
  expect(callback).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(1000));
  expect(callback).not.toHaveBeenCalled();
});

const debouncedConsumers = import.meta.glob<string>(
  [
    "../../../../packages/ui/src/settings/shuangpin-custom-editor.tsx",
    "../../../../packages/ui/src/core/use-settled-slider.ts",
    "../../../../packages/ui/src/dictionary/use-builtin-dictionary-entries.ts",
    "../../../../packages/ui/src/chat/chat-page.tsx",
  ],
  { eager: true, query: "?raw", import: "default" },
);

test.each(Object.entries(debouncedConsumers))("%s 复用公共防抖回调生命周期", (_path, source) => {
  expect(source).toMatch(/from "(?:\.\.\/core|\.)\/use-debounced-callback"/);
  expect(source).not.toContain("setTimeout(");
  expect(source).not.toContain("clearTimeout(");
});
