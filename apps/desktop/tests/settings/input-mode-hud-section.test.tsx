// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeHudSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("input mode HUD switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<InputModeHudSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("中英文切换提示"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("shortcut placement uses the enabled default and its shortcut label", () => {
  render(<InputModeHudSection shortcut value={undefined} onChange={vi.fn()} />);

  const toggle = screen.getByLabelText("切换中英文时显示提示") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
});

test("聚焦提示默认关闭且独立于切换提示保存", () => {
  const switchChange = vi.fn();
  const focusChange = vi.fn();
  render(
    <InputModeHudSection value={false} onChange={switchChange} focus={{ onChange: focusChange }} />,
  );
  const focus = screen.getByLabelText("聚焦输入框时提示") as HTMLInputElement;
  expect(focus.checked).toBe(false);
  fireEvent.click(focus);
  expect(focusChange).toHaveBeenCalledWith(true);
  expect(switchChange).not.toHaveBeenCalled();
});

test("未提供 Linux 聚焦能力时只显示切换提示", () => {
  render(<InputModeHudSection value={true} onChange={vi.fn()} />);
  expect(screen.queryByLabelText("聚焦输入框时提示")).toBeNull();
  expect(screen.queryByLabelText("屏幕角落模式提示")).toBeNull();
});

test("角落提示默认开启，关闭时不修改切换或聚焦提示", () => {
  const switchChange = vi.fn();
  const focusChange = vi.fn();
  const cornerChange = vi.fn();
  const { rerender } = render(
    <InputModeHudSection
      value={true}
      onChange={switchChange}
      focus={{ value: true, onChange: focusChange }}
      corner={{ onChange: cornerChange }}
    />,
  );
  const corner = screen.getByLabelText("屏幕角落模式提示") as HTMLInputElement;
  expect(corner.checked).toBe(true);
  fireEvent.click(corner);
  expect(cornerChange).toHaveBeenCalledWith(false);
  expect(switchChange).not.toHaveBeenCalled();
  expect(focusChange).not.toHaveBeenCalled();
  rerender(
    <InputModeHudSection
      value={true}
      onChange={switchChange}
      corner={{ value: false, onChange: cornerChange }}
    />,
  );
  expect((screen.getByLabelText("屏幕角落模式提示") as HTMLInputElement).checked).toBe(false);
});
