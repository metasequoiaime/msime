// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { useSettledSlider } from "../../../../packages/ui/src/core/use-settled-slider";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("拖动时立即预览，静止 300ms 后只保存最后一个值", () => {
  vi.useFakeTimers();
  const save = vi.fn();
  const { result } = renderHook(() => {
    const [value, setValue] = useState(600);
    return useSettledSlider(value, (next) => {
      save(next);
      setValue(next);
    });
  });
  act(() => result.current[1](700));
  expect(result.current[0]).toBe(700);
  act(() => vi.advanceTimersByTime(200));
  act(() => result.current[1](900));
  act(() => vi.advanceTimersByTime(299));
  expect(save).not.toHaveBeenCalled();
  expect(result.current[0]).toBe(900);
  act(() => vi.advanceTimersByTime(1));
  expect(save).toHaveBeenCalledExactlyOnceWith(900);
  expect(result.current[0]).toBe(900);
});

test("回到已保存的值时不重复保存", () => {
  vi.useFakeTimers();
  const save = vi.fn();
  const { result } = renderHook(() => useSettledSlider(3, save));
  act(() => result.current[1](6));
  act(() => result.current[1](3));
  act(() => vi.advanceTimersByTime(300));
  expect(save).not.toHaveBeenCalled();
  expect(result.current[0]).toBe(3);
});

test("保存回调更新时保持当前倒计时并使用最新回调", () => {
  vi.useFakeTimers();
  const first = vi.fn();
  const latest = vi.fn();
  const { result, rerender } = renderHook(({ save }) => useSettledSlider(3, save), {
    initialProps: { save: first },
  });
  act(() => result.current[1](5));
  act(() => vi.advanceTimersByTime(200));
  rerender({ save: latest });
  act(() => vi.advanceTimersByTime(100));
  expect(first).not.toHaveBeenCalled();
  expect(latest).toHaveBeenCalledExactlyOnceWith(5);
});

test("外部值变化时保留本地预览并重新等待，卸载后取消保存", () => {
  vi.useFakeTimers();
  const save = vi.fn();
  const { result, rerender, unmount } = renderHook(({ value }) => useSettledSlider(value, save), {
    initialProps: { value: 3 },
  });
  act(() => result.current[1](5));
  act(() => vi.advanceTimersByTime(200));
  rerender({ value: 4 });
  expect(result.current[0]).toBe(5);
  act(() => vi.advanceTimersByTime(299));
  expect(save).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(1));
  expect(save).toHaveBeenCalledExactlyOnceWith(5);
  act(() => result.current[1](7));
  unmount();
  act(() => vi.advanceTimersByTime(300));
  expect(save).toHaveBeenCalledOnce();
});
