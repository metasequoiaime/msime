// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { VoiceHotkeysSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("uses Windows labels and writes all three hold keys when the choice changes", () => {
  const onChange = vi.fn();
  render(
    <VoiceHotkeysSection
      platform="windows"
      values={{ hotkey_ralt: false, hotkey_ctrl_win: true }}
      onChange={onChange}
    />,
  );

  expect(screen.getByText(/输入法运行时全局生效。长按快捷键录音/)).toBeTruthy();
  expect(screen.getByRole("region", { name: "语音快捷键" })).toBeTruthy();
  const hold = screen.getByRole("combobox", { name: "长按录音" }) as HTMLSelectElement;
  // 右 Alt 关着，右 Ctrl+右 Alt 缺省按出厂值算关，所以显示 Ctrl+Win。
  expect(hold.value).toBe("hotkey_ctrl_win");
  expect(within(hold).getByRole("option", { name: "右 Alt" })).toBeTruthy();
  fireEvent.change(hold, { target: { value: "hotkey_ralt" } });
  expect(onChange).toHaveBeenCalledWith({
    hotkey_ralt: true,
    hotkey_rctrl_ralt: false,
    hotkey_ctrl_win: false,
  });
  fireEvent.change(hold, { target: { value: "none" } });
  expect(onChange).toHaveBeenLastCalledWith({
    hotkey_ralt: false,
    hotkey_rctrl_ralt: false,
    hotkey_ctrl_win: false,
  });
  fireEvent.click(screen.getByRole("switch", { name: "长按录音时按空格锁定" }));
  expect(onChange).toHaveBeenLastCalledWith({ hotkey_hold_space_lock: false });
});

// 缺的键按 client-core 的出厂值算：只有右 Alt 默认打开，另外两个缺省为关。只关掉右 Alt 的配置在宿主上没有任何按住的组合键，设置页显示「不使用」。
test("missing hold keys fall back to the host defaults", () => {
  render(
    <VoiceHotkeysSection platform="windows" values={{ hotkey_ralt: false }} onChange={vi.fn()} />,
  );
  expect((screen.getByRole("combobox", { name: "长按录音" }) as HTMLSelectElement).value).toBe(
    "none",
  );
});

// 以前三个组合键可以同时打开。这样的配置在用户改动之前照旧显示第一个，渲染时不改写存储。
test("a profile with several hold keys shows the first one and writes nothing", () => {
  const onChange = vi.fn();
  render(
    <VoiceHotkeysSection
      platform="macos"
      values={{ hotkey_ralt: false, hotkey_rctrl_ralt: true, hotkey_ctrl_win: true }}
      onChange={onChange}
    />,
  );
  expect((screen.getByRole("combobox", { name: "按住录音" }) as HTMLSelectElement).value).toBe(
    "hotkey_rctrl_ralt",
  );
  expect(onChange).not.toHaveBeenCalled();
});

test("uses macOS-specific labels and falls back to the host defaults for missing values", () => {
  render(<VoiceHotkeysSection platform="macos" values={{}} onChange={vi.fn()} />);

  const hold = screen.getByRole("combobox", { name: "按住录音" }) as HTMLSelectElement;
  expect(hold.value).toBe("hotkey_ralt");
  expect(within(hold).getByRole("option", { name: "右 Option" })).toBeTruthy();
  expect(within(hold).getByRole("option", { name: "Control+Command" })).toBeTruthy();
  expect(within(hold).getByRole("option", { name: "不使用" })).toBeTruthy();
  expect((screen.getByLabelText("空格锁定语音") as HTMLInputElement).checked).toBe(true);
  // Ctrl+F9 收在「更多选项」里。
  const f9 = screen.getByLabelText("Ctrl+F9 切换语音") as HTMLInputElement;
  expect(f9.checked).toBe(true);
  expect(f9.closest("details")).toBeTruthy();
});

// macOS 把空格锁定固定为开（`voice_space_lock_switch` 为 false），不给开关；没选按住的组合键时，空格锁定什么也不做，也不显示。
test("the space lock switch follows the host and the chosen hold key", () => {
  const { rerender } = render(
    <VoiceHotkeysSection platform="macos" values={{}} onChange={vi.fn()} spaceLock={false} />,
  );
  expect(screen.queryByLabelText("空格锁定语音")).toBeNull();
  rerender(
    <VoiceHotkeysSection
      platform="windows"
      values={{ hotkey_ralt: false, hotkey_rctrl_ralt: false, hotkey_ctrl_win: false }}
      onChange={vi.fn()}
    />,
  );
  expect((screen.getByRole("combobox", { name: "长按录音" }) as HTMLSelectElement).value).toBe(
    "none",
  );
  expect(screen.queryByLabelText("长按录音时按空格锁定")).toBeNull();
});
