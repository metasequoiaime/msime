// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceHotkeysSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("uses Windows labels and forwards shortcut changes", () => {
  const onChange = vi.fn();
  render(
    <VoiceHotkeysSection platform="windows" values={{ hotkey_ralt: false }} onChange={onChange} />,
  );

  expect(screen.getByText(/输入法运行时全局生效。长按快捷键录音/)).toBeTruthy();
  expect(screen.getByRole("region", { name: "语音快捷键" })).toBeTruthy();
  const ralt = screen.getByLabelText("长按右 Alt 录音") as HTMLInputElement;
  expect(ralt.checked).toBe(false);
  expect(screen.getByRole("switch", { name: "长按右 Alt 录音" })).toBe(ralt);
  fireEvent.click(ralt);
  expect(onChange).toHaveBeenCalledWith("hotkey_ralt", true);
});

test("uses macOS-specific labels and enables missing values by default", () => {
  render(<VoiceHotkeysSection platform="macos" values={{}} onChange={vi.fn()} />);

  expect(screen.getByLabelText("按住右 Option 录音")).toBeTruthy();
  expect(screen.getByLabelText("按住 Control+Command 录音")).toBeTruthy();
  expect((screen.getByLabelText("空格锁定语音") as HTMLInputElement).checked).toBe(true);
});
