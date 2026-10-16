// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useMacosInputModes,
  INPUT_MODE_RECHECK_MS,
  INPUT_MODE_RECHECK_WINDOW_MS,
  missingMacosInputModeEntries,
  groupMacosInputModeEntries,
  type MacosInputModesClient,
  type UseMacosInputModesOptions,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const prefix = "app.msime.synthetic.";
const options = { scheme: "cantonese" as const, inputSchemes: ["quanpin", "cantonese"] as const };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function client(): MacosInputModesClient {
  return { enabled: vi.fn(async () => [prefix + "Hans"]), openSettings: vi.fn(async () => {}) };
}

test("纯模型按宿主方案筛选缺失入口，按语言首次出现的顺序分组", () => {
  const missing = missingMacosInputModeEntries([prefix + "Hans"], options.inputSchemes);
  expect(missing.map((entry) => entry.mode)).toEqual(["Cantonese", "Roman"]);
  expect(groupMacosInputModeEntries(missing)).toEqual([
    { language: "粤语", entries: [missing[0]] },
    { language: "简体中文", entries: [missing[1]] },
  ]);
  expect(
    missingMacosInputModeEntries([prefix + "NotHans"], ["quanpin"]).map((entry) => entry.mode),
  ).toEqual(["Hans", "Roman"]);
});

test("公共状态独立读取列表、定位当前缺失方案并在焦点恢复时刷新", async () => {
  const current = client();
  const hook = renderHook(() =>
    useMacosInputModes({ ...options, client: current, onError: vi.fn() }),
  );
  await waitFor(() => expect(hook.result.current.current?.mode).toBe("Cantonese"));
  expect(hook.result.current.missing.map((entry) => entry.mode)).toEqual(["Cantonese", "Roman"]);
  expect(hook.result.current.groups.map((group) => group.language)).toEqual(["粤语", "简体中文"]);
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(current.enabled).toHaveBeenCalledTimes(2));
});

test("重叠读取只接受最新回答，读取失败后隐藏列表并能恢复", async () => {
  const first = deferred<string[] | null>();
  const second = deferred<string[] | null>();
  const current = client();
  vi.mocked(current.enabled).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const hook = renderHook(() =>
    useMacosInputModes({ ...options, client: current, onError: vi.fn() }),
  );
  act(() => {
    void hook.result.current.refresh();
  });
  await act(async () => second.resolve([prefix + "Hans", prefix + "Roman", prefix + "Cantonese"]));
  await act(async () => first.resolve([prefix + "Hans"]));
  expect(hook.result.current.missing).toEqual([]);
  vi.mocked(current.enabled).mockRejectedValueOnce(new Error("合成读取错误"));
  await act(async () => hook.result.current.refresh());
  expect(hook.result.current.enabled).toBeNull();
  expect(hook.result.current.missing).toEqual([]);
  await act(async () => hook.result.current.refresh());
  expect(hook.result.current.current?.mode).toBe("Cantonese");
});

test("展示参数变化只重新计算，旧客户端回答不能覆盖当前列表", async () => {
  const pending = deferred<string[] | null>();
  const previous = client();
  vi.mocked(previous.enabled).mockReturnValue(pending.promise);
  const current = client();
  const initialProps: UseMacosInputModesOptions = {
    ...options,
    client: previous,
    onError: vi.fn(),
  };
  const hook = renderHook((props: UseMacosInputModesOptions) => useMacosInputModes(props), {
    initialProps,
  });
  hook.rerender({ ...initialProps, client: current });
  await waitFor(() => expect(hook.result.current.enabled).toEqual([prefix + "Hans"]));
  await act(async () => pending.resolve([prefix + "Hans", prefix + "Roman", prefix + "Cantonese"]));
  expect(hook.result.current.current?.mode).toBe("Cantonese");
  hook.rerender({
    ...initialProps,
    client: current,
    scheme: "quanpin",
    inputSchemes: ["quanpin"],
    onError: vi.fn(),
  });
  expect(hook.result.current.current).toBeUndefined();
  expect(hook.result.current.missing.map((entry) => entry.mode)).toEqual(["Roman"]);
  expect(current.enabled).toHaveBeenCalledTimes(1);
});

test("打开系统设置后每三秒刷新两分钟，到期停止，焦点恢复仍能刷新", async () => {
  vi.useFakeTimers();
  const current = client();
  const onError = vi.fn();
  const hook = renderHook(() => useMacosInputModes({ ...options, client: current, onError }));
  await act(async () => {});
  await act(async () => hook.result.current.openSettings());
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_MODE_RECHECK_MS - 1));
  expect(current.enabled).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTimeAsync(1));
  expect(current.enabled).toHaveBeenCalledTimes(2);
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_MODE_RECHECK_WINDOW_MS));
  const count = vi.mocked(current.enabled).mock.calls.length;
  await act(async () => vi.advanceTimersByTimeAsync(INPUT_MODE_RECHECK_MS * 2));
  expect(current.enabled).toHaveBeenCalledTimes(count);
  act(() => window.dispatchEvent(new Event("focus")));
  expect(current.enabled).toHaveBeenCalledTimes(count + 1);
  expect(onError).not.toHaveBeenCalled();
  hook.unmount();
  expect(vi.getTimerCount()).toBe(0);
});

test("StrictMode 中打开动作同批防重，失败读取当前错误回调", async () => {
  const request = deferred<void>();
  const current = client();
  vi.mocked(current.openSettings).mockReturnValue(request.promise);
  const previousError = vi.fn();
  const currentError = vi.fn();
  const initialProps: UseMacosInputModesOptions = {
    ...options,
    client: current,
    onError: previousError,
  };
  const hook = renderHook((props: UseMacosInputModesOptions) => useMacosInputModes(props), {
    initialProps,
    wrapper: StrictMode,
  });
  act(() => {
    void hook.result.current.openSettings();
    void hook.result.current.openSettings();
  });
  expect(current.openSettings).toHaveBeenCalledTimes(1);
  expect(hook.result.current.opening).toBe(true);
  hook.rerender({ ...initialProps, onError: currentError });
  expect(hook.result.current.opening).toBe(true);
  await act(async () => request.reject(new Error("合成设置打开错误")));
  expect(previousError).not.toHaveBeenCalled();
  expect(currentError).toHaveBeenCalledExactlyOnceWith(
    "无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。",
  );
  expect(hook.result.current.opening).toBe(false);
});

test.each(["更换", "卸载"])("%s客户端后旧打开动作失败不报警", async (change) => {
  const request = deferred<void>();
  const previous = client();
  vi.mocked(previous.openSettings).mockReturnValue(request.promise);
  const onError = vi.fn();
  const initialProps: UseMacosInputModesOptions = { ...options, client: previous, onError };
  const hook = renderHook((props: UseMacosInputModesOptions) => useMacosInputModes(props), {
    initialProps,
  });
  act(() => {
    void hook.result.current.openSettings();
  });
  if (change === "卸载") hook.unmount();
  else hook.rerender({ ...initialProps, client: client() });
  await act(async () => request.reject(new Error("合成旧客户端错误")));
  expect(onError).not.toHaveBeenCalled();
});

test("没有客户端时不读取、不打开系统设置、不注册轮询", async () => {
  vi.useFakeTimers();
  const onError = vi.fn();
  const hook = renderHook(() => useMacosInputModes({ ...options, onError }));
  await act(async () => hook.result.current.refresh());
  await act(async () => hook.result.current.openSettings());
  expect(hook.result.current.enabled).toBeNull();
  expect(hook.result.current.missing).toEqual([]);
  expect(hook.result.current.opening).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
  expect(onError).not.toHaveBeenCalled();
});
