// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useVoicePanel, type VoicePanelClient } from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

type VoiceUpdate = Parameters<NonNullable<VoicePanelClient["onVoiceUpdate"]>>[0];

test("更换宿主隔离旧语言、识别和事件，晚到订阅立即释放", async () => {
  const language = deferred<string>();
  const recognition = deferred<{ text: string }>();
  const subscription = deferred<() => void>();
  const stopSubscription = vi.fn();
  let update!: VoiceUpdate;
  const first: VoicePanelClient = {
    close: vi.fn(),
    loadVoiceLanguage: () => language.promise,
    recognizeVoice: () => recognition.promise,
    cancelVoice: vi.fn().mockResolvedValue(undefined),
    onVoiceUpdate: (listener) => {
      update = listener;
      return subscription.promise;
    },
  };
  const hook = renderHook(({ client }) => useVoicePanel(client), {
    initialProps: { client: first },
  });
  let request!: Promise<void>;
  act(() => {
    request = hook.result.current.recognize();
    update({ text: "合成识别中", final: false });
  });
  expect(hook.result.current.text).toBe("合成识别中");
  const next: VoicePanelClient = {
    close: vi.fn(),
    loadVoiceLanguage: vi.fn().mockResolvedValue("en"),
  };
  hook.rerender({ client: next });
  await waitFor(() => expect(hook.result.current.language).toBe("en"));
  expect(first.cancelVoice).toHaveBeenCalledOnce();
  expect(hook.result.current.busy).toBe(false);
  await act(async () => {
    language.resolve("ja");
    recognition.resolve({ text: "合成旧结果" });
    subscription.resolve(stopSubscription);
    update({ text: "合成旧事件", final: true });
    await request;
  });
  expect(hook.result.current.language).toBe("en");
  expect(hook.result.current.text).toBe("合成识别中");
  expect(stopSubscription).toHaveBeenCalledOnce();
});

test("StrictMode 与卸载均释放迟到的语音订阅", async () => {
  const pending: ReturnType<typeof deferred<() => void>>[] = [];
  const client: VoicePanelClient = {
    close: vi.fn(),
    onVoiceUpdate: () => {
      const value = deferred<() => void>();
      pending.push(value);
      return value.promise;
    },
  };
  const hook = renderHook(() => useVoicePanel(client), {
    reactStrictMode: true,
  });
  expect(pending).toHaveLength(2);
  const stops = pending.map(() => vi.fn());
  await act(async () => pending[0].resolve(stops[0]));
  expect(stops[0]).toHaveBeenCalledOnce();
  hook.unmount();
  await act(async () => pending[1].resolve(stops[1]));
  expect(stops[1]).toHaveBeenCalledOnce();
});

test("提交保留新编辑，超限结果仍可复制，清空动作可独立调用", async () => {
  const sent = deferred<void>();
  const client: VoicePanelClient = {
    close: vi.fn(),
    maxSubmitBytes: 5,
    sendVoiceText: vi.fn(() => sent.promise),
    copyText: vi.fn().mockResolvedValue(undefined),
  };
  const hook = renderHook(() => useVoicePanel(client));
  act(() => hook.result.current.updateText("合成"));
  expect(hook.result.current.exceedsSubmitLimit).toBe(true);
  await act(async () => hook.result.current.submit());
  expect(client.sendVoiceText).not.toHaveBeenCalled();
  await act(async () => hook.result.current.submit(true));
  expect(client.copyText).toHaveBeenCalledWith("合成");
  expect(hook.result.current.text).toBe("合成");
  act(() => hook.result.current.updateText("old"));
  let first!: Promise<void>;
  act(() => {
    first = hook.result.current.submit();
    void hook.result.current.submit();
  });
  expect(client.sendVoiceText).toHaveBeenCalledExactlyOnceWith("old");
  expect(hook.result.current.submitting).toBe(true);
  act(() => hook.result.current.updateText("new"));
  await act(async () => {
    sent.resolve();
    await first;
  });
  expect(hook.result.current.text).toBe("new");
  expect(hook.result.current.notice).toBe("上一版内容已提交，当前内容已保留");
  act(() => hook.result.current.clear());
  expect(hook.result.current.text).toBe("");
  expect(hook.result.current.notice).toBe("识别结果已清空");
});

test("关闭先使识别失效，等待取消失败后仍关闭宿主", async () => {
  const recognition = deferred<{ text: string }>();
  const cancellation = deferred<void>();
  const client: VoicePanelClient = {
    close: vi.fn().mockResolvedValue(undefined),
    recognizeVoice: vi.fn(() => recognition.promise),
    cancelVoice: vi.fn(() => cancellation.promise),
  };
  const hook = renderHook(() => useVoicePanel(client));
  let recognizing!: Promise<void>;
  let closing!: Promise<void>;
  act(() => {
    recognizing = hook.result.current.recognize();
    closing = hook.result.current.close();
  });
  expect(hook.result.current.busy).toBe(false);
  expect(client.close).not.toHaveBeenCalled();
  await act(async () => {
    recognition.resolve({ text: "合成迟到结果" });
    await recognizing;
  });
  expect(hook.result.current.text).toBe("");
  await act(async () => {
    cancellation.reject(new Error("synthetic"));
    await closing;
  });
  expect(client.close).toHaveBeenCalledOnce();
});
