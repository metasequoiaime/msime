// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { copyAccountId } from "@msime/ui";

beforeEach(() => {
  vi.useFakeTimers();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

test("复制账号 ID 后只通知成功，提示时长交给调用方", async () => {
  const onCopied = vi.fn();
  const setError = vi.fn();
  copyAccountId({ id: "synthetic-account", isMounted: () => true, onCopied, setError });

  await vi.waitFor(() =>
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("synthetic-account"),
  );
  await vi.waitFor(() => expect(onCopied).toHaveBeenCalledOnce());
  expect(vi.getTimerCount()).toBe(0);
  expect(setError).not.toHaveBeenCalled();
});

test("挂载期间报告复制失败，卸载后忽略失败结果", async () => {
  const onCopied = vi.fn();
  const setError = vi.fn();
  let mounted = true;
  const writeText = vi.fn().mockRejectedValue(new Error("synthetic clipboard failure"));
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });

  copyAccountId({ id: "synthetic-account", isMounted: () => mounted, onCopied, setError });
  await vi.waitFor(() =>
    expect(setError).toHaveBeenCalledWith("账号 ID 暂时无法复制，请稍后重试。"),
  );
  mounted = false;
  copyAccountId({ id: "synthetic-account", isMounted: () => mounted, onCopied, setError });
  await vi.runAllTimersAsync();
  expect(onCopied).not.toHaveBeenCalled();
  expect(setError).toHaveBeenCalledOnce();
});

test("复制完成前卸载页面时不通知成功或启动提示计时器", async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  vi.mocked(navigator.clipboard.writeText).mockReturnValue(pending);
  let mounted = true;
  const onCopied = vi.fn();
  const setError = vi.fn();
  copyAccountId({ id: "synthetic-account", isMounted: () => mounted, onCopied, setError });

  mounted = false;
  finish();
  await vi.runAllTimersAsync();

  expect(onCopied).not.toHaveBeenCalled();
  expect(setError).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});
