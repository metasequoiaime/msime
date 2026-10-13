// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { KeyboardPanel } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("pointer hold repeats ordinary keys after the native delay and stops on release", async () => {
  vi.useFakeTimers();
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="macos" />);
  const key = screen.getByRole("button", { name: "a" });

  await act(async () => {
    fireEvent.pointerDown(key, { button: 0, isPrimary: true, pointerId: 1 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);

  await act(async () => {
    vi.advanceTimersByTime(449);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);

  await act(async () => {
    vi.advanceTimersByTime(151);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(4);

  fireEvent.pointerUp(key, { button: 0, isPrimary: true, pointerId: 1 });
  await act(async () => {
    vi.advanceTimersByTime(300);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(4);
});

test("modifier holds toggle only on activation and keyboard clicks remain single-shot", async () => {
  vi.useFakeTimers();
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="macos" />);
  const shift = screen.getAllByRole("button", { name: "Shift" })[0];

  fireEvent.pointerDown(shift, { button: 0, isPrimary: true, pointerId: 2 });
  await act(async () => {
    vi.advanceTimersByTime(900);
    await Promise.resolve();
  });
  expect(shift.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(shift, { detail: 1 });
  expect(shift.getAttribute("aria-pressed")).toBe("true");

  const letter = screen.getByRole("button", { name: "A" });
  await act(async () => {
    fireEvent.click(letter, { detail: 0 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);
  expect(sendKey).toHaveBeenCalledWith(expect.objectContaining({ virtual_key: 0x41, shift: true }));
});

test("Caps Lock and Shift invert letters while commit keys drop sticky modifiers", async () => {
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="windows" />);

  // Keys are delivered through a queue, so each press has to settle before the call is read --
  // asserting synchronously reads the previous key's request.
  const press = async (button: HTMLElement) => {
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });
  };

  // The reference panel draws the key faces from Shift alone -- Caps Lock does not change them --
  // while the character it posts follows caps != shift (KeyboardPanel.cpp, the face at the label
  // and the case at the post). So the face still reads "a" here and the keystroke is uppercase.
  fireEvent.click(screen.getByRole("button", { name: "Caps Lock" }));
  await press(screen.getByRole("button", { name: "a" }));
  expect(sendKey).toHaveBeenLastCalledWith(
    expect.objectContaining({ virtual_key: 0x41, shift: true }),
  );

  // Shift on top of Caps Lock: the face turns uppercase, the keystroke turns back to lowercase.
  fireEvent.click(screen.getAllByRole("button", { name: "Shift" })[0]);
  await press(screen.getByRole("button", { name: "A" }));
  expect(sendKey).toHaveBeenLastCalledWith(
    expect.objectContaining({ virtual_key: 0x41, shift: false }),
  );

  // Left and right Ctrl are both on the layout, same as Shift above.
  fireEvent.click(screen.getAllByRole("button", { name: "Ctrl" })[0]);
  await press(screen.getByRole("button", { name: "Enter" }));
  expect(sendKey).toHaveBeenLastCalledWith(
    expect.objectContaining({
      virtual_key: 0x0d,
      include_sticky_modifiers: false,
    }),
  );
});

test("Linux stops held-key retries after delivery failure and does not repeat Num Lock", async () => {
  vi.useFakeTimers();
  const sendKey = vi
    .fn()
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(new Error("synthetic"));
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="linux" />);
  const key = screen.getByRole("button", { name: "a" });
  fireEvent.pointerDown(key, { button: 0, isPrimary: true, pointerId: 3 });
  expect(sendKey).toHaveBeenCalledTimes(1);
  await act(async () => {
    vi.advanceTimersByTime(450);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("status").textContent).toContain("后续排队按键已取消");
  await act(async () => {
    vi.advanceTimersByTime(600);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(2);

  const numLock = screen.getByRole("button", { name: "Num Lock" });
  fireEvent.pointerDown(numLock, { button: 0, isPrimary: true, pointerId: 4 });
  fireEvent.click(numLock, { detail: 1 });
  await act(async () => {
    vi.advanceTimersByTime(900);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(3);
});

test("Windows sends a key on release over the same key, like the shipped panel's mouse-up", async () => {
  vi.useFakeTimers();
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="windows" />);
  const a = screen.getByRole("button", { name: "a" });
  const s = screen.getByRole("button", { name: "s" });

  // Pressing does nothing yet, and holding never repeats.
  await act(async () => {
    fireEvent.pointerDown(a, { button: 0, isPrimary: true, pointerId: 1 });
    vi.advanceTimersByTime(900);
    await Promise.resolve();
  });
  expect(sendKey).not.toHaveBeenCalled();

  // Sliding off and back on still sends, as KeyboardPanel::OnMouseUp compares only the key under the release.
  fireEvent.pointerLeave(a, { pointerId: 1 });
  await act(async () => {
    fireEvent.pointerUp(a, { button: 0, isPrimary: true, pointerId: 1 });
    fireEvent.click(a, { detail: 1 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);
  expect(sendKey).toHaveBeenCalledWith(expect.objectContaining({ virtual_key: 0x41 }));

  // Released over a different key: nothing is sent for either key.
  await act(async () => {
    fireEvent.pointerDown(a, { button: 0, isPrimary: true, pointerId: 2 });
    fireEvent.pointerUp(s, { button: 0, isPrimary: true, pointerId: 2 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);

  // A cancelled pointer sends nothing either.
  await act(async () => {
    fireEvent.pointerDown(s, { button: 0, isPrimary: true, pointerId: 3 });
    fireEvent.pointerCancel(s, { pointerId: 3 });
    fireEvent.pointerUp(s, { button: 0, isPrimary: true, pointerId: 3 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);
});

test.each([
  ["permission_denied", "辅助功能"],
  ["no_input_target", "先点击其他应用的输入框"],
  ["input_target_changed", "本次按键未发送"],
])("macOS explains %s and resumes only after a fresh press", async (code, message) => {
  vi.useFakeTimers();
  const sendKey = vi.fn().mockRejectedValueOnce({ code }).mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} platform="macos" />);
  expect(screen.getByRole("status").textContent).toContain("先点击目标应用的输入框");
  const key = screen.getByRole("button", { name: "a" });
  await act(async () => {
    fireEvent.pointerDown(key, { button: 0, isPrimary: true, pointerId: 21 });
    await Promise.resolve();
  });
  expect(screen.getByRole("status").textContent).toContain(message);
  await act(async () => {
    vi.advanceTimersByTime(1000);
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(1);
  await act(async () => {
    fireEvent.click(key, { detail: 0 });
    await Promise.resolve();
  });
  expect(sendKey).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("status").textContent).toContain("已发送");
});

test("macOS explains an initial target capture failure without exposing raw errors", async () => {
  const rememberInputTarget = vi.fn().mockRejectedValue({
    code: "no_input_target",
    detail: "synthetic-private-detail",
  });
  await act(async () => {
    render(
      <KeyboardPanel client={{ close: async () => {}, rememberInputTarget }} platform="macos" />,
    );
  });
  expect(screen.getByRole("status").textContent).toContain("先点击其他应用的输入框");
  expect(screen.getByRole("status").textContent).not.toContain("synthetic-private-detail");
});
