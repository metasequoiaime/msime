// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { InputSourceStartupStatus } from "@msime/ui";
import {
  INPUT_SOURCE_RECHECK_MS,
  useMacosSettings,
} from "../../../../packages/ui/src/settings/use-macos-settings";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const waiting: InputSourceStartupStatus = {
  action: "up_to_date",
  enabled: false,
  bundled_version: "1.0.0",
  installed_version: "1.0.0",
};
const ready: InputSourceStartupStatus = { ...waiting, enabled: true };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("输入源慢读取不会被下一轮重查作废，添加成功后停止轮询", async () => {
  vi.useFakeTimers();
  const pending = deferred<InputSourceStartupStatus | null>();
  const status = vi.fn().mockResolvedValueOnce(waiting).mockReturnValue(pending.promise);
  const client = { inputSourceStartup: { status, openSettings: vi.fn() } };
  const hook = renderHook(() => useMacosSettings({ client, macos: true, setError: vi.fn() }));
  await act(async () => {});
  expect(hook.result.current.inputSourceStartup).toEqual(waiting);
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_SOURCE_RECHECK_MS * 3));
  expect(status).toHaveBeenCalledTimes(2);
  await act(async () => pending.resolve(ready));
  expect(hook.result.current.inputSourceStartup).toEqual(ready);
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_SOURCE_RECHECK_MS * 3));
  expect(status).toHaveBeenCalledTimes(2);
});

test.each(["成功", "失败"])("主动重查接管后，旧%s不释放新请求占位", async (outcome) => {
  vi.useFakeTimers();
  const previous = deferred<InputSourceStartupStatus | null>();
  const pending = deferred<InputSourceStartupStatus | null>();
  const status = vi
    .fn()
    .mockResolvedValue(waiting)
    .mockResolvedValueOnce(waiting)
    .mockReturnValueOnce(previous.promise)
    .mockReturnValueOnce(pending.promise);
  const client = { inputSourceStartup: { status, openSettings: vi.fn() } };
  const hook = renderHook(() => useMacosSettings({ client, macos: true, setError: vi.fn() }));
  await act(async () => {});
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_SOURCE_RECHECK_MS));
  act(() => {
    void hook.result.current.refreshInputSourceStartup();
  });
  expect(status).toHaveBeenCalledTimes(3);
  await act(async () => {
    if (outcome === "成功") previous.resolve(ready);
    else previous.reject(new Error("合成旧状态读取失败"));
  });
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_SOURCE_RECHECK_MS));
  expect(status).toHaveBeenCalledTimes(3);
  expect(hook.result.current.inputSourceStartup).toEqual(waiting);
  await act(async () => pending.resolve(ready));
  expect(hook.result.current.inputSourceStartup).toEqual(ready);
});
