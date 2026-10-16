// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useVoiceMicrophonePicker, useVoiceDevicePicker, type VoiceCaptureDevice } from "@msime/ui";

afterEach(cleanup);
const devices: VoiceCaptureDevice[] = [
  { backend: "macos", id: "synthetic-id", label: "合成麦克风" },
  { backend: "pulse", id: "synthetic-id", label: "合成 Pulse 设备" },
];
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("单后端在可见时自动读取，默认选择显式清除，不因选择回调变化丢失列表", async () => {
  const read = vi.fn(async () => devices);
  const choose = vi.fn();
  const props = { read, active: false, device: "synthetic-id", backendSupported: true, choose };
  const { result, rerender } = renderHook((options) => useVoiceMicrophonePicker(options), {
    initialProps: props,
  });
  expect(read).not.toHaveBeenCalled();
  expect(result.current.value).toBe("stored");
  rerender({ ...props, active: true });
  await waitFor(() => expect(result.current.loaded).toBe(true));
  expect(result.current.value).toBe("0");
  expect(choose).not.toHaveBeenCalled();
  act(() => result.current.select("0"));
  expect(choose).toHaveBeenLastCalledWith("macos", "synthetic-id");
  act(() => result.current.select(""));
  expect(choose).toHaveBeenLastCalledWith("", "");
  const nextChoose = vi.fn();
  rerender({ ...props, active: true, choose: nextChoose });
  expect(result.current.devices).toEqual(devices);
  expect(read).toHaveBeenCalledOnce();
  act(() => result.current.select("1"));
  expect(nextChoose).toHaveBeenCalledWith("pulse", "synthetic-id");
});

test("单后端区分外来后端与断开的设备，失败后恢复可再次读取", async () => {
  const read = vi
    .fn()
    .mockRejectedValueOnce(new Error("synthetic failure"))
    .mockResolvedValue(devices);
  const props = {
    read,
    active: true,
    device: "synthetic-missing",
    backendSupported: true,
    choose: vi.fn(),
  };
  const { result, rerender } = renderHook((options) => useVoiceMicrophonePicker(options), {
    initialProps: props,
  });
  await waitFor(() => expect(result.current.error).not.toBe(""));
  expect(result.current.value).toBe("stored");
  expect(result.current.busy).toBe(false);
  rerender({ ...props, active: false, backendSupported: false });
  expect(result.current.value).toBe("foreign");
  act(() => result.current.select("foreign"));
  expect(props.choose).not.toHaveBeenCalled();
  rerender({ ...props, active: true, device: "" });
  await waitFor(() => expect(result.current.loaded).toBe(true));
  expect(result.current.value).toBe("");
  expect(result.current.error).toBe("");
});

test("单后端更换读取器清理旧设备、完成与失败状态", async () => {
  const initial = vi.fn(async () => devices);
  const pending = deferred<VoiceCaptureDevice[]>();
  const replacement = vi.fn(() => pending.promise);
  const props = { active: true, device: "synthetic-id", backendSupported: true, choose: vi.fn() };
  const { result, rerender } = renderHook(
    ({ read }) => useVoiceMicrophonePicker({ ...props, read }),
    { initialProps: { read: initial } },
  );
  await waitFor(() => expect(result.current.loaded).toBe(true));
  rerender({ read: replacement });
  expect(result.current.devices).toEqual([]);
  expect(result.current.loaded).toBe(false);
  expect(result.current.error).toBe("");
  expect(result.current.value).toBe("stored");
  await act(async () => {
    pending.resolve([]);
  });
  expect(result.current.loaded).toBe(true);
});

test.each(["success", "failure"])("单后端旧读取 %s 不覆盖新读取或解除当前忙碌", async (outcome) => {
  const oldRead = deferred<VoiceCaptureDevice[]>();
  const nextRead = deferred<VoiceCaptureDevice[]>();
  const initial = vi.fn(() => oldRead.promise);
  const replacement = vi.fn(() => nextRead.promise);
  const props = { active: true, device: "", backendSupported: true, choose: vi.fn() };
  const { result, rerender } = renderHook(
    ({ read }) => useVoiceMicrophonePicker({ ...props, read }),
    { initialProps: { read: initial }, wrapper: StrictMode },
  );
  rerender({ read: replacement });
  await act(async () => {
    if (outcome === "success") oldRead.resolve(devices);
    else oldRead.reject(new Error("synthetic failure"));
  });
  expect(result.current.devices).toEqual([]);
  expect(result.current.error).toBe("");
  expect(result.current.loaded).toBe(false);
  expect(result.current.busy).toBe(true);
  await act(async () => {
    nextRead.resolve([]);
  });
  expect(result.current.loaded).toBe(true);
  expect(result.current.busy).toBe(false);
});

test("多后端只手动刷新，按后端和标识共同匹配并只转交有效选项", async () => {
  const read = vi.fn(async () => devices);
  const choose = vi.fn();
  const { result } = renderHook(() =>
    useVoiceDevicePicker({ read, backend: "pulse", device: "synthetic-id", choose }),
  );
  expect(read).not.toHaveBeenCalled();
  expect(result.current.selected).toBe(-1);
  await act(async () => {
    await result.current.refresh();
  });
  expect(result.current.selected).toBe(1);
  expect(result.current.notice).toBe("选择设备后从下一次录音生效");
  act(() => {
    result.current.select("");
    result.current.select("invalid");
  });
  expect(choose).not.toHaveBeenCalled();
  act(() => result.current.select("1"));
  expect(choose).toHaveBeenCalledExactlyOnceWith("pulse", "synthetic-id");
});

test("多后端同批刷新只有一次读取，空列表和错误均释放忙碌并允许重试", async () => {
  const pending = deferred<VoiceCaptureDevice[]>();
  const read = vi
    .fn()
    .mockReturnValueOnce(pending.promise)
    .mockRejectedValueOnce(new Error("synthetic failure"))
    .mockResolvedValueOnce(devices);
  const { result } = renderHook(() =>
    useVoiceDevicePicker({ read, backend: "", device: "", choose: vi.fn() }),
  );
  let first!: Promise<void> | undefined;
  act(() => {
    first = result.current.refresh();
    result.current.refresh();
  });
  expect(read).toHaveBeenCalledOnce();
  await act(async () => {
    pending.resolve([]);
    await first;
  });
  expect(result.current.notice).toBe("未发现设备，可手动填写设备名称或使用默认设备");
  await act(async () => {
    await result.current.refresh();
  });
  expect(result.current.notice).toBe("无法读取设备列表，可重试或手动填写");
  expect(result.current.busy).toBe(false);
  await act(async () => {
    await result.current.refresh();
  });
  expect(result.current.devices).toEqual(devices);
});

test.each(["success", "failure"])("多后端切换读取器后旧 %s 不覆盖新列表及提示", async (outcome) => {
  const oldRead = deferred<VoiceCaptureDevice[]>();
  const initial = vi.fn(() => oldRead.promise);
  const replacement = vi.fn(async () => devices);
  const props = { backend: "", device: "", choose: vi.fn() };
  const { result, rerender } = renderHook(({ read }) => useVoiceDevicePicker({ ...props, read }), {
    initialProps: { read: initial },
    wrapper: StrictMode,
  });
  let oldAction!: Promise<void> | undefined;
  act(() => {
    oldAction = result.current.refresh();
  });
  rerender({ read: replacement });
  expect(replacement).not.toHaveBeenCalled();
  expect(result.current.notice).toBe("点击刷新读取可用录音设备");
  expect(result.current.busy).toBe(false);
  await act(async () => {
    await result.current.refresh();
  });
  await act(async () => {
    if (outcome === "success") oldRead.resolve([]);
    else oldRead.reject(new Error("synthetic failure"));
    await oldAction;
  });
  expect(result.current.devices).toEqual(devices);
  expect(result.current.notice).toBe("选择设备后从下一次录音生效");
});
