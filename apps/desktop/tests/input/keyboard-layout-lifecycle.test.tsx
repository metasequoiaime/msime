// @vitest-environment jsdom
import { StrictMode } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { KeyboardPanel } from "@msime/ui";

const storageKey = "msime.keyboard.layout";
const client = { close: async () => {} };
const currentLayout = () =>
  screen.getByRole("main", { name: "屏幕键盘" }).getAttribute("data-keyboard-layout");
const toggle = () => fireEvent.click(screen.getByRole("button", { name: "切换键盘布局" }));
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

test("saved layout survives initial effects and StrictMode replay", () => {
  localStorage.setItem(storageKey, "nine_key");
  render(
    <StrictMode>
      <KeyboardPanel client={client} />
    </StrictMode>,
  );
  expect(currentLayout()).toBe("nine_key");
  expect(screen.queryByRole("button", { name: /^q$/ })).toBeNull();
});

test("user layout survives closing and reopening the panel", () => {
  const view = render(<KeyboardPanel client={client} />);
  toggle();
  expect(currentLayout()).toBe("nine_key");
  expect(localStorage.getItem(storageKey)).toBe("nine_key");
  view.unmount();
  render(<KeyboardPanel client={client} />);
  expect(currentLayout()).toBe("nine_key");
});

test("host layout changes apply without overriding later user toggles on unrelated rerenders", () => {
  const view = render(<KeyboardPanel client={client} layout="twenty_six_key" />);
  view.rerender(<KeyboardPanel client={client} layout="nine_key" />);
  expect(currentLayout()).toBe("nine_key");
  toggle();
  expect(currentLayout()).toBe("twenty_six_key");
  view.rerender(<KeyboardPanel client={client} layout="nine_key" theme="light" />);
  expect(currentLayout()).toBe("twenty_six_key");
  view.rerender(<KeyboardPanel client={client} layout="twenty_six_key" />);
  view.rerender(<KeyboardPanel client={client} layout="nine_key" />);
  expect(currentLayout()).toBe("nine_key");
});

test("unavailable storage does not prevent layout switching", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("synthetic");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("synthetic");
  });
  render(<KeyboardPanel client={client} />);
  toggle();
  expect(currentLayout()).toBe("nine_key");
});

test("invalid saved layout falls back to the host layout", () => {
  localStorage.setItem(storageKey, "invalid");
  render(<KeyboardPanel client={client} layout="nine_key" />);
  expect(currentLayout()).toBe("nine_key");
});

test("nine-key shuangpin shows host key faces while sending digits", async () => {
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(
    <KeyboardPanel
      client={{ close: async () => {}, sendKey }}
      layout="nine_key_shuangpin"
      nineKeyShuangpinKeys={[{ key: "1", initials: ["b"], finals: ["a"], zero_initial: false }]}
    />,
  );
  const one = screen.getByRole("button", { name: "1b · a" });
  expect(one.textContent).toContain("b");
  expect(one.textContent).toContain("a");
  fireEvent.click(one);
  await vi.waitFor(() =>
    expect(sendKey).toHaveBeenCalledWith(
      expect.objectContaining({ virtual_key: 0x31, shift: false }),
    ),
  );
});
