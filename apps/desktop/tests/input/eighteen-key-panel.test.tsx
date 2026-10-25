// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useKeyboardPanel, type PanelClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

test("Windows 十八键覆盖全部字母并发送显式组码，切换时清掉修饰键", async () => {
  const sendKey = vi.fn(async (_request: Parameters<NonNullable<PanelClient["sendKey"]>>[0]) => {});
  const client = { close: async () => {}, sendKey };
  const { result } = renderHook(() => useKeyboardPanel({ client, platform: "windows" }));
  expect(result.current.activeLayout).toBe("twenty_six_key");
  act(() => result.current.pressKey({ label: "Ctrl", virtualKey: 0x11, modifier: "Ctrl" }));
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("eighteen_key");
  expect(result.current.renderedModifiers.size).toBe(0);
  const groups = result.current.rows.flat().filter((item) => item.groupedLetter);
  expect(groups).toHaveLength(18);
  expect(
    groups
      .flatMap((item) => [...item.label])
      .sort()
      .join(""),
  ).toBe("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
  const bn = groups.find((item) => item.label === "BN")!;
  await act(async () => result.current.pressKey(bn));
  expect(sendKey).toHaveBeenLastCalledWith({
    virtual_key: 0x42,
    grouped_letter: "b",
    shift: false,
    modifiers: { ctrl: false, alt: false, win: false },
    include_sticky_modifiers: false,
  });
  const enter = result.current.rows.flat().find((item) => item.label === "Enter")!;
  await act(async () => result.current.pressKey(enter));
  expect(sendKey).toHaveBeenLastCalledWith(expect.objectContaining({ virtual_key: 0x0d }));
  expect(sendKey.mock.calls.at(-1)?.[0]).not.toHaveProperty("grouped_letter");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("nine_key");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("twenty_six_key");
});

test("其他平台不恢复 Windows 十八键布局", () => {
  localStorage.setItem("msime.keyboard.layout", "eighteen_key");
  const client = { close: async () => {} };
  const { result } = renderHook(() => useKeyboardPanel({ client, platform: "macos" }));
  expect(result.current.activeLayout).toBe("twenty_six_key");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("nine_key");
});

test("十八键投递不确定时清空后续队列，不能自动重发", async () => {
  localStorage.setItem("msime.keyboard.layout", "eighteen_key");
  const sendKey = vi.fn().mockRejectedValue({ code: "grouped_key_unconfirmed" });
  const client = { close: async () => {}, sendKey };
  const { result } = renderHook(() => useKeyboardPanel({ client, platform: "windows" }));
  const bn = result.current.rows.flat().find((item) => item.label === "BN")!;
  await act(async () => {
    result.current.pressKey(bn);
    result.current.pressKey(bn);
  });
  expect(sendKey).toHaveBeenCalledTimes(1);
  expect(result.current.notice).toContain("请先检查输入框");
});

test("Windows 增加十八键入口后仍能切回宿主选择的九键双拼", () => {
  localStorage.setItem("msime.keyboard.layout", "twenty_six_key");
  const client = { close: async () => {} };
  const { result } = renderHook(() =>
    useKeyboardPanel({ client, platform: "windows", layout: "nine_key_shuangpin" }),
  );
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("eighteen_key");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("nine_key_shuangpin");
});
