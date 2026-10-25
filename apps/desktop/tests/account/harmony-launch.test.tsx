// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HarmonyLaunch } from "../../../../apps/harmony/src/launch";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function launch(mobile = true, page = "设置", dark = false) {
  return (
    <StrictMode>
      <HarmonyLaunch mobile={mobile} appTheme={null} dark={dark}>
        <h1>{page}</h1>
      </HarmonyLaunch>
    </StrictMode>
  );
}

test("手机每个新文档均播放开屏，1.4 秒后显示目标页面", () => {
  vi.useFakeTimers();
  const first = render(launch());
  expect(screen.getByRole("main", { name: "启动动画" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: "设置" })).toBeNull();
  act(() => vi.advanceTimersByTime(1399));
  expect(screen.getByRole("button", { name: "跳过开屏" })).toBeTruthy();
  act(() => vi.advanceTimersByTime(1));
  expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();
  first.unmount();

  render(launch());
  expect(screen.getByRole("main", { name: "启动动画" })).toBeTruthy();
});

test("跳过后切页面、恢复前台或重放引导不再出现开屏", () => {
  vi.useFakeTimers();
  const view = render(launch());
  fireEvent.click(screen.getByRole("button", { name: "跳过开屏" }));
  expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();
  fireEvent(document, new Event("visibilitychange"));
  fireEvent(window, new Event("focus"));
  view.rerender(launch(true, "首次设置", true));
  expect(screen.getByRole("heading", { name: "首次设置" })).toBeTruthy();
  expect(screen.queryByRole("main", { name: "启动动画" })).toBeNull();
  act(() => vi.advanceTimersByTime(1400));
  expect(screen.queryByRole("button", { name: "跳过开屏" })).toBeNull();
});

test("二合一不显示手机开屏", () => {
  render(launch(false));
  expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "跳过开屏" })).toBeNull();
});

test.each([
  [false, "#F1F3F5"],
  [true, "#0A0B0A"],
])("开屏系统栏跟随外观，dark=%s", (dark, background) => {
  const setSystemBars = vi.fn();
  render(
    <HarmonyLaunch mobile appTheme={null} chrome={{ setSystemBars }} dark={dark}>
      <h1>设置</h1>
    </HarmonyLaunch>,
  );
  expect(setSystemBars).toHaveBeenCalledWith({ background, navigationBar: background, dark });
});
