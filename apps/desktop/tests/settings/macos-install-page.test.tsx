// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  MacosInstallPage,
  nextSimulatedProgress,
  type InputSourceStartupStatus,
  type MacosInstallClient,
} from "@msime/ui";

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const status = (overrides: Partial<InputSourceStartupStatus>): InputSourceStartupStatus => ({
  action: "installed",
  enabled: false,
  bundled_version: "0.50.0 (1)",
  installed_version: "0.50.0 (1)",
  ...overrides,
});

/** Lets React render between ticks, as it does between real ones: the page reacts to the bar reaching 100 in an effect. */
async function advance(ms: number) {
  for (let elapsed = 0; elapsed < ms; elapsed += 100) {
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
  }
}

function client(result: () => Promise<InputSourceStartupStatus>): MacosInstallClient {
  return { install: vi.fn(result) };
}

test("simulated progress holds short of 100 until the install has finished", () => {
  let progress = 0;
  for (let tick = 0; tick < 1000; tick += 1) progress = nextSimulatedProgress(progress, false, 0.9);
  expect(progress).toBeGreaterThan(90);
  expect(progress).toBeLessThan(100);
  expect(nextSimulatedProgress(40, false, 0.05)).toBe(40);
  expect(nextSimulatedProgress(95, true, 0.5)).toBe(100);
});

test("the install button turns into a progress bar and then hands over to the settings page", async () => {
  const onComplete = vi.fn();
  render(<MacosInstallPage client={client(async () => status({}))} onComplete={onComplete} />);
  expect(screen.getByText("适用于 macOS 13 及以上版本")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
  const bar = screen.getByRole("progressbar", { name: "正在安装" });
  await advance(1000);
  const midway = Number(bar.getAttribute("aria-valuenow"));
  expect(midway).toBeGreaterThan(0);
  expect(midway).toBeLessThan(100);
  await advance(6000);
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("安装完成");
  expect(screen.getByText(/按页面上的提示把它添加到系统的输入法列表/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "进入设置" }));
  expect(onComplete).toHaveBeenCalled();
});

test("the bar does not finish before the install does", async () => {
  let finish: (value: InputSourceStartupStatus) => void = () => undefined;
  const pending = new Promise<InputSourceStartupStatus>((resolve) => {
    finish = resolve;
  });
  render(<MacosInstallPage client={client(() => pending)} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
  await advance(60_000);
  const held = Number(screen.getByRole("progressbar").getAttribute("aria-valuenow"));
  expect(held).toBeLessThan(100);
  await act(async () => {
    finish(status({ enabled: true }));
  });
  await advance(2000);
  expect(screen.getByRole("status").textContent).toBe("安装完成");
  expect(screen.getByText(/按 Control\+空格/)).toBeTruthy();
});

test("a failed install offers a retry", async () => {
  const install = client(() => Promise.reject(new Error("unavailable")));
  render(<MacosInstallPage client={install} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
  await advance(6000);
  expect(screen.getByRole("alert").textContent).toBe("安装没有完成");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  expect(install.install).toHaveBeenCalledTimes(2);
});

test("an install the session cannot see yet says a login comes first", async () => {
  render(
    <MacosInstallPage
      client={client(async () => status({ action: "login_required" }))}
      onComplete={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
  await advance(6000);
  expect(screen.getByText(/注销并重新登录后/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "进入设置" })).toBeTruthy();
});
