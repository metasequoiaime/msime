// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import {
  MobileKeyboardFeedbackSection,
  MobileKeyboardFeedbackSettings,
  type MobileKeyboardFeedback,
  useMobileKeyboardFeedback,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const value: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: true,
  hapticStrength: "medium",
  englishSuggestions: true,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("updates sound, haptics, and strength", () => {
  const onChange = vi.fn();
  render(
    <MobileKeyboardFeedbackSection
      value={value}
      busy={false}
      ios={false}
      canPreview
      onChange={onChange}
      onPreview={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "按键音" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, soundEnabled: false });

  fireEvent.click(screen.getByRole("switch", { name: "按键振动" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, hapticsEnabled: false });

  fireEvent.change(screen.getByRole("combobox", { name: "振动强度" }), {
    target: { value: "strong" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ ...value, hapticStrength: "strong" });

  fireEvent.change(screen.getByRole("combobox", { name: "振动强度" }), {
    target: { value: "system" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ ...value, hapticStrength: "system" });
  expect(screen.getByRole("option", { name: "跟随系统" })).toBeTruthy();
});

test("shows 按键弹出预览 only where the host's keyboard reports it", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <MobileKeyboardFeedbackSection
      value={value}
      busy={false}
      ios={false}
      canPreview
      onChange={onChange}
      onPreview={vi.fn()}
    />,
  );
  expect(screen.queryByRole("switch", { name: "按键弹出预览" })).toBeNull();

  const withPopup: MobileKeyboardFeedback = { ...value, keyPopup: true };
  rerender(
    <MobileKeyboardFeedbackSection
      value={withPopup}
      busy={false}
      ios={false}
      canPreview
      onChange={onChange}
      onPreview={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("switch", { name: "按键弹出预览" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...withPopup, keyPopup: false });
});

test("offers preview and iOS English suggestions", () => {
  const onChange = vi.fn();
  const onPreview = vi.fn();
  render(
    <MobileKeyboardFeedbackSection
      value={value}
      busy={false}
      ios
      canPreview
      onChange={onChange}
      onPreview={onPreview}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "试一下振动" }));
  expect(onPreview).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("switch", { name: "英文建议" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, englishSuggestions: false });
});

test("hides vibration controls when unavailable", () => {
  render(
    <MobileKeyboardFeedbackSection
      value={{ ...value, hapticsAvailable: false }}
      busy={false}
      ios={false}
      canPreview
      onChange={vi.fn()}
      onPreview={vi.fn()}
    />,
  );

  expect(screen.queryByRole("switch", { name: "按键振动" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "振动强度" })).toBeNull();
});

test("a save response from a replaced mobile feedback client is ignored", async () => {
  const pendingSave = deferred<MobileKeyboardFeedback>();
  const pendingLoad = deferred<MobileKeyboardFeedback>();
  const oldClient = {
    load: vi.fn().mockResolvedValue(value),
    save: vi.fn().mockReturnValue(pendingSave.promise),
  };
  const nextValue = { ...value, soundEnabled: false };
  const nextClient = {
    load: vi.fn().mockReturnValue(pendingLoad.promise),
    save: vi.fn().mockResolvedValue(nextValue),
  };
  const onError = vi.fn();
  const { result, rerender } = renderHook(
    ({ client }) => useMobileKeyboardFeedback({ mobile: true, client, onError }),
    { initialProps: { client: oldClient } },
  );
  await waitFor(() => expect(result.current.value).toEqual(value));

  let pending!: Promise<void>;
  act(() => {
    pending = result.current.save({ ...value, hapticsEnabled: false });
  });
  rerender({ client: nextClient });
  pendingLoad.resolve(nextValue);
  await waitFor(() => expect(result.current.value).toEqual(nextValue));

  pendingSave.resolve({ ...value, hapticsEnabled: false });
  await act(async () => pending);
  expect(result.current.value).toEqual(nextValue);
});

test("clears the previous mobile feedback while a replacement client is loading", async () => {
  const pendingLoad = deferred<MobileKeyboardFeedback>();
  const oldClient = {
    load: vi.fn().mockResolvedValue(value),
    save: vi.fn().mockResolvedValue(value),
  };
  const nextClient = {
    load: vi.fn().mockReturnValue(pendingLoad.promise),
    save: vi.fn().mockResolvedValue(value),
  };
  const { result, rerender } = renderHook(
    ({ client }) => useMobileKeyboardFeedback({ mobile: true, client, onError: vi.fn() }),
    { initialProps: { client: oldClient } },
  );
  await waitFor(() => expect(result.current.value).toEqual(value));

  rerender({ client: nextClient });

  expect(result.current.value).toBeUndefined();
  expect(result.current.busy).toBe(true);

  pendingLoad.resolve(value);
  await waitFor(() => expect(result.current.value).toEqual(value));
  expect(result.current.busy).toBe(false);
});

test("a late mobile feedback preview failure is ignored after unmount", async () => {
  let rejectPreview!: (reason: unknown) => void;
  const pendingPreview = new Promise<void>((_resolve, reject) => {
    rejectPreview = reject;
  });
  const preview = vi.fn().mockReturnValue(pendingPreview);
  const onError = vi.fn();
  const client = {
    load: vi.fn().mockResolvedValue(value),
    save: vi.fn().mockResolvedValue(value),
    preview,
  };
  const { result, unmount } = renderHook(() =>
    useMobileKeyboardFeedback({ mobile: true, client, onError }),
  );
  await waitFor(() => expect(result.current.value).toEqual(value));

  let pending!: Promise<void>;
  act(() => {
    pending = result.current.preview();
  });
  onError.mockClear();
  unmount();
  rejectPreview(new Error("fixture failure"));
  await act(async () => pending);
  expect(onError).not.toHaveBeenCalled();
});

test("ignores a same-tick duplicate mobile feedback save", async () => {
  const pendingSave = deferred<MobileKeyboardFeedback>();
  const save = vi.fn().mockReturnValue(pendingSave.promise);
  const client = {
    load: vi.fn().mockResolvedValue(value),
    save,
  };
  const { result } = renderHook(() =>
    useMobileKeyboardFeedback({ mobile: true, client, onError: vi.fn() }),
  );
  await waitFor(() => expect(result.current.value).toEqual(value));

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.save({ ...value, hapticsEnabled: false });
    second = result.current.save({ ...value, hapticsEnabled: false });
  });
  expect(save).toHaveBeenCalledOnce();
  pendingSave.resolve({ ...value, hapticsEnabled: false });
  await act(async () => {
    await first;
    await second;
  });
});

test("restores the previous value and reports a failed mobile feedback save", async () => {
  const save = vi.fn().mockRejectedValue(new Error("fixture failure"));
  const onError = vi.fn();
  const client = {
    load: vi.fn().mockResolvedValue(value),
    save,
  };
  const { result } = renderHook(() => useMobileKeyboardFeedback({ mobile: true, client, onError }));
  await waitFor(() => expect(result.current.value).toEqual(value));
  onError.mockClear();

  await act(async () => {
    await result.current.save({ ...value, hapticsEnabled: false });
  });

  expect(result.current.value).toEqual(value);
  expect(onError).toHaveBeenLastCalledWith("无法保存按键反馈设置，请重试。");
});

test("only mounts the host binding when mobile feedback is available", () => {
  const value: MobileKeyboardFeedback = {
    soundEnabled: true,
    hapticsEnabled: true,
    hapticStrength: "medium",
  };
  const client = { load: async () => value, save: async () => value, preview: vi.fn() };
  const { rerender } = render(
    <MobileKeyboardFeedbackSettings
      mobile={false}
      client={client}
      value={value}
      busy={false}
      ios={false}
      onChange={vi.fn()}
      onPreview={vi.fn()}
    />,
  );
  expect(screen.queryByRole("group", { name: "按键反馈" })).toBeNull();

  rerender(
    <MobileKeyboardFeedbackSettings
      mobile
      client={client}
      value={value}
      busy={false}
      ios={false}
      onChange={vi.fn()}
      onPreview={vi.fn()}
    />,
  );
  expect(screen.getByRole("group", { name: "按键反馈" })).toBeTruthy();
});
