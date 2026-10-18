// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useKeyboardPanel, type PanelClient } from "@msime/ui";

function deferred() {
  let resolve!: () => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<void>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

const a = { label: "a", virtualKey: 0x41 };
const b = { label: "b", virtualKey: 0x42 };
const shift = { label: "Shift", virtualKey: 0x10, modifier: "Shift" as const };

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.restoreAllMocks();
});

test("StrictMode 保留已存布局，宿主变更后用户仍可切换九键双拼", () => {
  localStorage.setItem("msime.keyboard.layout", "nine_key");
  const client = { close: async () => {} };
  const { result, rerender } = renderHook(
    ({ layout }: { layout: "twenty_six_key" | "nine_key_shuangpin" }) =>
      useKeyboardPanel({ client, layout }),
    { initialProps: { layout: "twenty_six_key" }, wrapper: StrictMode },
  );
  expect(result.current.activeLayout).toBe("nine_key");
  rerender({ layout: "nine_key_shuangpin" });
  expect(result.current.activeLayout).toBe("nine_key_shuangpin");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("twenty_six_key");
  act(() => result.current.switchLayout());
  expect(result.current.activeLayout).toBe("nine_key_shuangpin");
  expect(localStorage.getItem("msime.keyboard.layout")).toBe("nine_key_shuangpin");
});

test("目标捕获和按键串行执行，单次 Shift 只作用于下一键", async () => {
  const pending = deferred();
  const events: string[] = [];
  const rememberInputTarget = vi.fn(async () => {
    events.push("target");
  });
  const sendKey = vi.fn(async (request) => {
    events.push(`send:${request.virtual_key}`);
    if (request.virtual_key === 0x41) await pending.promise;
  });
  const client = { close: async () => {}, rememberInputTarget, sendKey };
  const { result } = renderHook(() => useKeyboardPanel({ client }));
  await act(async () => undefined);
  events.length = 0;
  await act(async () => {
    result.current.pressKey(shift);
    result.current.pressKey(a);
    result.current.pressKey(b);
  });
  expect(events).toEqual(["target", "send:65"]);
  expect(result.current.renderedModifiers.has("Shift")).toBe(false);
  expect(sendKey).toHaveBeenNthCalledWith(1, {
    virtual_key: 0x41,
    shift: true,
    modifiers: { ctrl: false, alt: false, win: false },
    include_sticky_modifiers: true,
  });
  await act(async () => pending.resolve());
  expect(events).toEqual(["target", "send:65", "target", "send:66"]);
  expect(sendKey).toHaveBeenLastCalledWith(expect.objectContaining({ shift: false }));
  expect(result.current.notice).toBe("已发送：b");
});

test("失败取消后续排队按键且不重放，重新按键可以继续发送", async () => {
  const pending = deferred();
  const sendKey = vi
    .fn()
    .mockImplementationOnce(() => pending.promise)
    .mockResolvedValue(undefined);
  const client = { close: async () => {}, sendKey };
  const { result } = renderHook(() => useKeyboardPanel({ client, platform: "linux" }));
  act(() => {
    result.current.pressKey(a);
    result.current.pressKey(b);
  });
  await act(async () => pending.reject(new Error("synthetic")));
  expect(sendKey).toHaveBeenCalledTimes(1);
  expect(result.current.notice).toContain("后续排队按键已取消");
  await act(async () => result.current.pressKey(b));
  expect(sendKey).toHaveBeenCalledTimes(2);
  expect(result.current.notice).toBe("已发送：b");
});

test("客户端替换后丢弃旧队列与修饰键，旧发送完成不覆盖新通知", async () => {
  const pending = deferred();
  const oldSend = vi.fn(() => pending.promise);
  const newSend = vi.fn().mockResolvedValue(undefined);
  const first: PanelClient = { close: async () => {}, sendKey: oldSend };
  const second: PanelClient = { close: async () => {}, sendKey: newSend };
  const { result, rerender } = renderHook(({ client }) => useKeyboardPanel({ client }), {
    initialProps: { client: first },
    wrapper: StrictMode,
  });
  act(() => {
    result.current.pressKey(a);
    result.current.pressKey(b);
    result.current.pressKey(shift);
  });
  rerender({ client: second });
  expect(result.current.renderedModifiers.size).toBe(0);
  await act(async () => result.current.pressKey(b));
  await act(async () => pending.resolve());
  expect(oldSend).toHaveBeenCalledTimes(1);
  expect(newSend).toHaveBeenCalledTimes(1);
  expect(result.current.notice).toBe("已发送：b");
});

test("语音等待按键完成，打开时不再发送，关闭失败后恢复输入", async () => {
  const keyPending = deferred();
  const voicePending = deferred();
  const sendKey = vi
    .fn()
    .mockImplementationOnce(() => keyPending.promise)
    .mockResolvedValue(undefined);
  const openVoice = vi.fn(() => voicePending.promise);
  const close = vi.fn().mockRejectedValue(new Error("synthetic"));
  const client = { close, sendKey, openVoice };
  const { result } = renderHook(() => useKeyboardPanel({ client }));
  act(() => result.current.pressKey(a));
  await act(async () => result.current.openVoiceKeyboard());
  expect(openVoice).not.toHaveBeenCalled();
  expect(result.current.notice).toContain("请等待按键发送完成");
  await act(async () => keyPending.resolve());
  act(() => {
    void result.current.openVoiceKeyboard();
    void result.current.openVoiceKeyboard();
    result.current.pressKey(b);
  });
  expect(result.current.openingVoice).toBe(true);
  expect(openVoice).toHaveBeenCalledTimes(1);
  expect(sendKey).toHaveBeenCalledTimes(1);
  await act(async () => voicePending.resolve());
  expect(result.current.openingVoice).toBe(false);
  await act(async () => result.current.closeKeyboard());
  expect(result.current.notice).toBe("无法关闭键盘，请重试");
  await act(async () => result.current.pressKey(b));
  expect(sendKey).toHaveBeenCalledTimes(2);
  expect(result.current.notice).toBe("已发送：b");
});
