// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { KeyboardPanel } from "@msime/ui";
import { DesktopKeyboard } from "../../src/input/desktop-keyboard";
import { invoke, isTauri } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn(() => false), invoke: vi.fn() }));

afterEach(() => {
  cleanup();
  localStorage.clear();
});

test("a standalone keyboard discovers its native platform without mounting SettingsPage", async () => {
  vi.mocked(isTauri).mockReturnValueOnce(true);
  vi.mocked(invoke).mockResolvedValueOnce({ platform: "macos" });
  render(
    <DesktopKeyboard
      client={{ close: async () => {} }}
      preferences={{
        load: vi.fn().mockRejectedValue(new Error("synthetic missing settings")),
      }}
    />,
  );
  await waitFor(() =>
    expect(screen.getAllByRole("button", { name: "Command" }).length).toBeGreaterThan(0),
  );
  expect(invoke).toHaveBeenCalledWith("host_capabilities");
});

test("macOS uses Command and Option while retaining the shared modifier contract", async () => {
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(
    <DesktopKeyboard
      client={{ close: async () => {}, sendKey }}
      preferences={{
        host: testHost({ platform: "macos" }),
        load: vi.fn().mockRejectedValue(new Error("synthetic missing settings")),
      }}
    />,
  );
  expect(screen.queryByRole("button", { name: "Win" })).toBeNull();
  for (const name of ["PrtSc", "Scroll", "Pause", "Ins", "Menu", "Num Lock"]) {
    expect(screen.queryByRole("button", { name })).toBeNull();
  }
  expect(screen.getByRole("button", { name: "Clear" })).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: "Command" })[0]);
  fireEvent.click(screen.getAllByRole("button", { name: "Option" })[0]);
  fireEvent.click(screen.getByRole("button", { name: "a" }));
  await waitFor(() =>
    expect(sendKey).toHaveBeenCalledWith({
      virtual_key: 0x41,
      shift: false,
      modifiers: { ctrl: false, alt: true, win: true },
      include_sticky_modifiers: true,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Enter" }));
  await waitFor(() =>
    expect(sendKey).toHaveBeenLastCalledWith(
      expect.objectContaining({
        virtual_key: 0x0d,
        include_sticky_modifiers: false,
      }),
    ),
  );
});

test("macOS modifier faces survive Shift updates and layout changes", () => {
  render(<KeyboardPanel platform="macos" client={{ close: async () => {} }} />);
  fireEvent.click(screen.getAllByRole("button", { name: "Shift" })[0]);
  expect(screen.getAllByRole("button", { name: "Command" }).length).toBeGreaterThan(0);
  expect(screen.getAllByRole("button", { name: "Option" }).length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button", { name: "切换键盘布局" }));
  expect(screen.getByRole("main", { name: "屏幕键盘" }).getAttribute("data-keyboard-layout")).toBe(
    "nine_key",
  );
});

test("Windows retains its PC key faces", () => {
  render(<KeyboardPanel platform="windows" client={{ close: async () => {} }} />);
  expect(screen.getAllByRole("button", { name: "Win" }).length).toBeGreaterThan(0);
  expect(screen.getByRole("button", { name: "PrtSc" })).toBeTruthy();
});
