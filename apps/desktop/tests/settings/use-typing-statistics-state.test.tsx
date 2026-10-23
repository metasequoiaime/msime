// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useTypingStatisticsState } from "../../../../packages/ui/src/settings/use-typing-statistics-state";
import type { TypingStatisticsStatus } from "../../../../packages/ui/src/settings/typing-statistics-model";
import type { TypingSummary } from "../../../../packages/ui/src/settings/typing-summary";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 5, 17, 12));
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function status(total = 4): TypingStatisticsStatus {
  return {
    statistics: { enabled: true, total, days: { "2026-06-17": total } },
    availability: "ready",
  };
}

function summary(total = 4): TypingSummary {
  return {
    overview: {
      week_total: total,
      previous_week_total: 0,
      last7: [{ day: "2026-06-17", count: total }],
      average_speed: null,
      previous_average_speed: null,
      first_candidate_rate: null,
      keystrokes_saved_rate: null,
      current_streak: 1,
      longest_streak: 1,
    },
    habits: {
      weeks12: [],
      hours24: Array(24).fill(0),
      usual_hours: null,
      peak_window: null,
      active_days: 1,
      breakdown: { characters: { han: total }, sources: { quanpin: total } },
    },
    keys: {
      per_character_keys: null,
      previous_per_character_keys: null,
      backspace_rate: null,
      prediction_rate: null,
      longest_run: null,
      positions: null,
    },
    achievements: [],
  };
}

function client() {
  return {
    load: vi.fn(async () => status()),
    setEnabled: vi.fn(async () => status()),
    reset: vi.fn(async () => status(0)),
  };
}

test("可见读取保留最短间隔与相同快照引用，卸载后停止订阅", async () => {
  const source = client();
  const hook = renderHook(() => useTypingStatisticsState(source));
  await act(async () => {});
  const first = hook.result.current.status;
  const today = hook.result.current.today;
  expect(first).toEqual(status());
  await act(async () => window.dispatchEvent(new Event("focus")));
  expect(source.load).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTimeAsync(1_000));
  await act(async () => window.dispatchEvent(new Event("focus")));
  expect(source.load).toHaveBeenCalledTimes(2);
  expect(hook.result.current.status).toBe(first);
  expect(hook.result.current.today).toBe(today);

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.load).toHaveBeenCalledTimes(2);
  hook.unmount();
  expect(vi.getTimerCount()).toBe(0);
  await act(async () => window.dispatchEvent(new Event("focus")));
  expect(source.load).toHaveBeenCalledTimes(2);
});

test("在途读取只在超过十五秒后被替换，旧回答不能覆盖新快照", async () => {
  const pending = deferred<TypingStatisticsStatus>();
  const source = client();
  source.load.mockReturnValueOnce(pending.promise).mockResolvedValue(status(8));
  const hook = renderHook(() => useTypingStatisticsState(source));
  await act(async () => vi.advanceTimersByTimeAsync(15_000));
  expect(source.load).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTimeAsync(1));
  await act(async () => window.dispatchEvent(new Event("focus")));
  expect(source.load).toHaveBeenCalledTimes(2);
  expect(hook.result.current.status).toEqual(status(8));
  await act(async () => pending.resolve(status(99)));
  expect(hook.result.current.status).toEqual(status(8));
});

test("写入防重，失败保留快照并允许重试，客户端更换后丢弃旧写入", async () => {
  const source = client();
  const hook = renderHook(({ source }) => useTypingStatisticsState(source), {
    initialProps: { source },
  });
  await act(async () => {});
  const failure = deferred<TypingStatisticsStatus>();
  source.setEnabled.mockReturnValueOnce(failure.promise);
  act(() => {
    void hook.result.current.update(source.setEnabled);
    void hook.result.current.update(source.reset);
  });
  expect(hook.result.current.busy).toBe(true);
  expect(source.reset).not.toHaveBeenCalled();
  await act(async () => failure.reject(new Error("合成写入失败")));
  expect(hook.result.current.status).toEqual(status());
  expect(hook.result.current.error).toContain("原有统计不会被自动重置");
  expect(hook.result.current.busy).toBe(false);
  await act(async () => hook.result.current.update(source.setEnabled));
  expect(hook.result.current.error).toBe("");

  const previous = deferred<TypingStatisticsStatus>();
  source.reset.mockReturnValueOnce(previous.promise);
  act(() => void hook.result.current.update(source.reset));
  const next = client();
  next.load.mockResolvedValue(status(12));
  hook.rerender({ source: next });
  await act(async () => {});
  await act(async () => previous.resolve(status(99)));
  expect(hook.result.current.status).toEqual(status(12));
  expect(hook.result.current.busy).toBe(false);
});

test("摘要读取期间统计变脏时，在回答后补读一次", async () => {
  const pending = deferred<TypingSummary>();
  const initial = summary();
  const latest = summary(8);
  const source = { ...client(), summary: vi.fn(async () => latest) };
  source.summary.mockReturnValueOnce(pending.promise);
  const hook = renderHook(() => useTypingStatisticsState(source, "harmony"));
  await act(async () => {});
  expect(source.summary).toHaveBeenCalledTimes(1);
  source.load.mockResolvedValue(status(8));
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.summary).toHaveBeenCalledTimes(1);
  await act(async () => pending.resolve(initial));
  expect(source.summary).toHaveBeenCalledTimes(2);
  expect(hook.result.current.summary).toBe(latest);
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.summary).toHaveBeenCalledTimes(2);
});

test("摘要失败在下一次轮询重试，跨午夜无新统计时仍重算摘要和日期", async () => {
  vi.setSystemTime(new Date(2026, 5, 17, 23, 59, 48));
  const source = { ...client(), summary: vi.fn(async () => summary()) };
  source.summary.mockRejectedValueOnce(new Error("合成摘要失败"));
  const hook = renderHook(() => useTypingStatisticsState(source, "harmony"));
  await act(async () => {});
  expect(hook.result.current.summaryFailed).toBe(true);
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.summary).toHaveBeenCalledTimes(2);
  expect(hook.result.current.summaryFailed).toBe(false);
  const first = hook.result.current.summary;
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.summary).toHaveBeenCalledTimes(2);
  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  expect(source.summary).toHaveBeenCalledTimes(3);
  expect(hook.result.current.today.key).toBe("2026-06-18");
  expect(hook.result.current.summary).toBe(first);
});

test("外观标识变化重建原轮询相位，没有摘要外观时不请求摘要", async () => {
  const source = { ...client(), summary: vi.fn(async () => summary()) };
  const initialProps: { look?: string } = {};
  const hook = renderHook(({ look }: { look?: string }) => useTypingStatisticsState(source, look), {
    initialProps,
  });
  await act(async () => {});
  expect(source.summary).not.toHaveBeenCalled();
  hook.rerender({ look: "harmony" });
  await act(async () => vi.advanceTimersByTimeAsync(4_000));
  hook.rerender({ look: "hm2" });
  await act(async () => {});
  expect(source.load).toHaveBeenCalledTimes(2);
  expect(source.summary).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTimeAsync(1_000));
  expect(source.load).toHaveBeenCalledTimes(2);
  await act(async () => vi.advanceTimersByTimeAsync(4_000));
  expect(source.load).toHaveBeenCalledTimes(3);
});

test("打开目录失败使用同一错误通道，重试清错且不重读统计", async () => {
  const source = { ...client(), openDirectory: vi.fn(async () => {}) };
  source.openDirectory.mockRejectedValueOnce(new Error("合成文件管理器失败"));
  const hook = renderHook(() => useTypingStatisticsState(source));
  await act(async () => {});
  await act(async () => hook.result.current.openDataDirectory());
  expect(hook.result.current.error).toBe("无法打开数据目录，可能是文件管理器不可用。");
  expect(hook.result.current.status).toEqual(status());
  expect(hook.result.current.busy).toBe(false);
  await act(async () => hook.result.current.openDataDirectory());
  expect(hook.result.current.error).toBe("");
  expect(source.openDirectory).toHaveBeenCalledTimes(2);
  expect(source.load).toHaveBeenCalledTimes(1);
});
