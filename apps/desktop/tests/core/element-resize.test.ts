// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { subscribeToElementResize } from "../../../../packages/ui/src/core/element-resize";

afterEach(() => vi.unstubAllGlobals());

test("观察指定元素，转发尺寸通知并在清理时断开", () => {
  const observe = vi.fn();
  const disconnect = vi.fn();
  let notify!: ResizeObserverCallback;
  const observer = { observe, disconnect };
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        notify = callback;
        return observer;
      }
    },
  );
  const element = document.createElement("div");
  const listener = vi.fn();
  const stop = subscribeToElementResize(element, listener);

  expect(observe).toHaveBeenCalledWith(element);
  expect(listener).not.toHaveBeenCalled();
  const entries: ResizeObserverEntry[] = [
    {
      target: element,
      contentRect: element.getBoundingClientRect(),
      borderBoxSize: [],
      contentBoxSize: [],
      devicePixelContentBoxSize: [],
    },
  ];
  notify(entries, observer as unknown as ResizeObserver);
  expect(listener).toHaveBeenCalledWith(entries, observer);
  stop?.();
  expect(disconnect).toHaveBeenCalledOnce();
});

test("缺少观察 API 时返回未注册，由调用方决定回退", () => {
  vi.stubGlobal("ResizeObserver", undefined);
  const listener = vi.fn();
  expect(subscribeToElementResize(document.createElement("div"), listener)).toBeUndefined();
  expect(listener).not.toHaveBeenCalled();
});
