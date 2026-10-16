// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { KeyboardPanel, type HostCapabilities, type Snapshot } from "@msime/ui";
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

// 生产里的键盘面板窗口没有现成的宿主能力，要等外壳的 `host_capabilities`。偏好先到时也不能先画出存下的间距：macOS 上那是已经砍掉的设置，画出来再跳回默认就是一次闪动。
test.each([
  ["macos", false, ["6px", "7px"]],
  ["linux", true, ["3.5px", "9.5px"]],
] as const)(
  "the standalone %s keyboard holds the default spacing until the shell reports the host",
  async (platform, spacing, settled) => {
    const stored: Snapshot = {
      format_version: 1,
      revision: 1,
      preferences: {
        scheme: "quanpin",
        shuangpin_profile: "xiaohe",
        candidate_page_size: 6,
        learning: true,
        chinese_punctuation: true,
        touch_key_spacing_tenths: 35,
        touch_row_spacing_tenths: 95,
      },
    };
    let report!: (value: unknown) => void;
    vi.mocked(isTauri).mockReturnValueOnce(true);
    vi.mocked(invoke).mockReturnValueOnce(
      new Promise((accept) => {
        report = accept;
      }),
    );
    const load = vi.fn(async () => stored);
    render(<DesktopKeyboard client={{ close: async () => {} }} preferences={{ load }} />);
    const gaps = () => {
      const grid = screen
        .getByRole("main", { name: "屏幕键盘" })
        .querySelector<HTMLDivElement>("[data-keyboard-layout-grid]");
      return [
        grid?.style.getPropertyValue("--keyboard-key-gap"),
        grid?.style.getPropertyValue("--keyboard-row-gap"),
      ];
    };
    await waitFor(() => expect(load).toHaveBeenCalledOnce());
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("host_capabilities");
    expect(gaps()).toEqual(["6px", "7px"]);
    await act(async () => report({ platform, touch_keyboard_spacing: spacing }));
    expect(gaps()).toEqual(settled);
  },
);

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
  const control = screen.getAllByRole("button", { name: "Ctrl" })[0];
  const command = screen.getAllByRole("button", { name: "Command" })[0];
  const option = screen.getAllByRole("button", { name: "Option" })[0];
  expect(control.compareDocumentPosition(option) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(option.compareDocumentPosition(command) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  const rightCommand = screen.getAllByRole("button", { name: "Command" })[1];
  const rightOption = screen.getAllByRole("button", { name: "Option" })[1];
  expect(
    rightCommand.compareDocumentPosition(rightOption) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
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

// 面板窗口的客户端不带宿主能力，要等外壳答复 `host_capabilities`。答复到来之前不能套用存下的屏幕键盘明暗，否则桌面面板会先按已经砍掉的单独设置闪一下；宿主报告各界面不跟随（`surface_themes_follow` 为假）时在答复后照旧套用。
test.each([
  ["macos", true, "light"],
  ["windows", true, "light"],
  ["linux", true, "light"],
  ["windows", false, "dark"],
] as const)(
  "a %s panel (surfaces follow: %s) never draws the stored keyboard theme before the host answers",
  async (platform, follow, answered) => {
    let answer!: (value: HostCapabilities) => void;
    vi.mocked(isTauri).mockReturnValueOnce(true);
    vi.mocked(invoke).mockReturnValueOnce(
      new Promise((done) => {
        answer = done;
      }),
    );
    const value: Snapshot = {
      format_version: 1,
      revision: 1,
      preferences: {
        scheme: "quanpin",
        shuangpin_profile: "xiaohe",
        candidate_page_size: 6,
        learning: true,
        chinese_punctuation: true,
        theme: "light",
        screen_keyboard_theme: "dark",
      },
    };
    const load = vi.fn(async () => value);
    render(<DesktopKeyboard client={{ close: async () => {} }} preferences={{ load }} />);
    const theme = () =>
      screen.getByRole("main", { name: "屏幕键盘" }).getAttribute("data-keyboard-theme");
    await waitFor(() => expect(load).toHaveBeenCalledOnce());
    await waitFor(() => expect(theme()).toBe("light"));
    await act(async () => answer(testHost({ platform, surface_themes_follow: follow })));
    expect(theme()).toBe(answered);
  },
);
