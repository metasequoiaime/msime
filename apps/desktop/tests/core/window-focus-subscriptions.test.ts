// @vitest-environment jsdom
import { act } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as windowEvents from "../../../../packages/ui/src/core/window-focus";
import {
  subscribeToWindowEvent,
  subscribeToWindowBlur,
  subscribeToWindowFocus,
  subscribeToWindowResume,
} from "../../../../packages/ui/src/core/window-focus";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("文档可见性订阅转发隐藏与显示事件，分别清理订阅", () => {
  const subscribe = windowEvents.subscribeToDocumentVisibilityChange;
  expect(subscribe).toBeTypeOf("function");
  const seen: DocumentVisibilityState[] = [];
  const other = vi.fn();
  const stop = subscribe(() => seen.push(document.visibilityState));
  const stopOther = subscribe(other);
  expect(seen).toEqual([]);

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  document.dispatchEvent(new Event("visibilitychange"));
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  document.dispatchEvent(new Event("visibilitychange"));
  expect(seen).toEqual(["hidden", "visible"]);
  expect(other).toHaveBeenCalledTimes(2);

  stop();
  stop();
  document.dispatchEvent(new Event("visibilitychange"));
  expect(seen).toEqual(["hidden", "visible"]);
  expect(other).toHaveBeenCalledTimes(3);
  stopOther();
  document.dispatchEvent(new Event("visibilitychange"));
  expect(other).toHaveBeenCalledTimes(3);
});

test("没有文档时可见性订阅返回可重复调用的空清理函数", () => {
  const subscribe = windowEvents.subscribeToDocumentVisibilityChange;
  expect(subscribe).toBeTypeOf("function");
  const callback = vi.fn();
  vi.stubGlobal("document", undefined);
  const stop = subscribe(callback);
  expect(() => {
    stop();
    stop();
  }).not.toThrow();
  expect(callback).not.toHaveBeenCalled();
});

test.each([
  ["focus", subscribeToWindowFocus, "blur"],
  ["blur", subscribeToWindowBlur, "focus"],
] as const)("%s 订阅只响应对应事件并独立清理", (event, subscribe, ignoredEvent) => {
  const callback = vi.fn();
  const otherCallback = vi.fn();
  const stop = subscribe(callback);
  const stopOther = subscribe(otherCallback);

  act(() => window.dispatchEvent(new Event(ignoredEvent)));
  expect(callback).not.toHaveBeenCalled();
  expect(otherCallback).not.toHaveBeenCalled();
  act(() => window.dispatchEvent(new Event(event)));
  expect(callback).toHaveBeenCalledOnce();
  expect(otherCallback).toHaveBeenCalledOnce();

  stop();
  act(() => window.dispatchEvent(new Event(event)));
  expect(callback).toHaveBeenCalledOnce();
  expect(otherCallback).toHaveBeenCalledTimes(2);
  stopOther();
});

test("通用窗口事件订阅按事件类型注册并独立清理", () => {
  const resize = vi.fn<(event: Event) => void>();
  const keydown = vi.fn<(event: KeyboardEvent) => void>();
  const stopResize = subscribeToWindowEvent("resize", resize);
  const stopKeydown = subscribeToWindowEvent("keydown", keydown);

  const resizeEvent = new Event("resize");
  const keydownEvent = new KeyboardEvent("keydown", { key: "Escape" });
  window.dispatchEvent(resizeEvent);
  window.dispatchEvent(keydownEvent);
  expect(resize).toHaveBeenCalledWith(resizeEvent);
  expect(keydown).toHaveBeenCalledWith(keydownEvent);

  stopResize();
  stopResize();
  window.dispatchEvent(resizeEvent);
  window.dispatchEvent(keydownEvent);
  expect(resize).toHaveBeenCalledOnce();
  expect(keydown).toHaveBeenCalledTimes(2);
  stopKeydown();
});

test("通用窗口事件订阅支持只触发一次", () => {
  const callback = vi.fn();
  const stop = subscribeToWindowEvent("popstate", callback, { once: true });
  const first = new PopStateEvent("popstate");
  const second = new PopStateEvent("popstate");

  window.dispatchEvent(first);
  window.dispatchEvent(second);

  expect(callback).toHaveBeenCalledOnce();
  expect(callback).toHaveBeenCalledWith(first);
  stop();
});

test("通用窗口事件订阅支持自定义事件名", () => {
  const callback = vi.fn();
  const stop = subscribeToWindowEvent("msime-test-event", callback);
  const event = new CustomEvent("msime-test-event", { detail: "合成详情" });

  window.dispatchEvent(event);

  expect(callback).toHaveBeenCalledWith(event);
  stop();
});

test("resumes on focus and visible transitions, but ignores hidden transitions", () => {
  const refresh = vi.fn();
  const stop = subscribeToWindowResume(refresh);

  act(() => window.dispatchEvent(new Event("focus")));
  expect(refresh).toHaveBeenCalledOnce();

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledOnce();

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledTimes(2);

  stop();
  act(() => window.dispatchEvent(new Event("focus")));
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledTimes(2);
});

const blurConsumers = import.meta.glob<string>(
  [
    "../../../../packages/ui/src/keyboard/use-keyboard-panel.ts",
    "../../../../packages/ui/src/keyboard/use-panel-drag.ts",
    "../../../../packages/ui/src/settings/use-flush-on-window-leave.ts",
    "../../../../packages/ui/src/settings/use-settings-window-interactions.ts",
  ],
  { eager: true, query: "?raw", import: "default" },
);

test.each(Object.entries(blurConsumers))("%s 复用公共失焦订阅生命周期", (_path, source) => {
  expect(source).toContain("subscribeToWindowBlur");
  expect(source).not.toContain('window.addEventListener("blur"');
  expect(source).not.toContain('window.removeEventListener("blur"');
});

test("窗口事件调用方复用公共注册与清理", () => {
  const sources = Object.values(
    import.meta.glob<string>(
      [
        "../../../../packages/ui/src/keyboard/handwriting-candidate-button.tsx",
        "../../../../packages/ui/src/settings/use-flush-on-window-leave.ts",
      ],
      { eager: true, query: "?raw", import: "default" },
    ),
  );
  expect(sources.every((source) => source.includes("subscribeToWindowEvent"))).toBe(true);
  expect(sources.every((source) => !source.includes('window.addEventListener("resize"'))).toBe(
    true,
  );
  expect(sources.every((source) => !source.includes('window.addEventListener("keydown"'))).toBe(
    true,
  );
  expect(sources.every((source) => !source.includes('window.addEventListener("pagehide"'))).toBe(
    true,
  );
});
