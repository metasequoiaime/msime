// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";
import type { KeyboardKeyButtonProps } from "@msime/ui";

afterEach(cleanup);

function props(overrides: Partial<KeyboardKeyButtonProps> = {}): KeyboardKeyButtonProps {
  return {
    keyToRender: { label: "a", virtualKey: 0x41 },
    modifiers: new Set(),
    material: "flat",
    flexGrow: 1,
    disabled: false,
    onPointerDown: vi.fn(),
    onPointerUp: vi.fn(),
    onPointerCancel: vi.fn(),
    onPointerLeave: vi.fn(),
    onActivate: vi.fn(),
    ...overrides,
  };
}

test("公共入口提供屏幕键盘按键组件", () => {
  expect(ui.KeyboardKeyButton).toBeTypeOf("function");
});

test("Shift 只改变单字符键面，Caps Lock 不改变键面并保留提示结构", () => {
  const state = props({ keyToRender: { label: "a", hint: "合成提示", virtualKey: 0x41 } });
  const view = render(<ui.KeyboardKeyButton {...state} />);
  const button = screen.getByRole("button");
  expect(button.querySelector("[data-key-label]")?.textContent).toBe("a");
  expect(screen.getByText("合成提示").parentElement).toBe(button);
  expect(button.hasAttribute("aria-pressed")).toBe(false);
  view.rerender(<ui.KeyboardKeyButton {...state} modifiers={new Set(["Caps Lock"])} />);
  expect(button.querySelector("[data-key-label]")?.textContent).toBe("a");
  view.rerender(<ui.KeyboardKeyButton {...state} modifiers={new Set(["Shift"])} />);
  expect(button.querySelector("[data-key-label]")?.textContent).toBe("A");
  view.rerender(
    <ui.KeyboardKeyButton
      {...state}
      keyToRender={{ label: "1", shifted: "!", virtualKey: 0x31 }}
      modifiers={new Set(["Shift"])}
    />,
  );
  expect(button.textContent).toBe("!");
  view.rerender(
    <ui.KeyboardKeyButton
      {...state}
      keyToRender={{ label: "Enter", shifted: "合成替代", virtualKey: 0x0d }}
      modifiers={new Set(["Shift"])}
    />,
  );
  expect(button.textContent).toBe("Enter");
});

test.each(["flat", "raised", "glass", "paper"] as const)(
  "%s 材质保留样式、比例和修饰键按下状态",
  (material) => {
    const state = props({
      keyToRender: { label: "Shift", virtualKey: 0x10, modifier: "Shift" },
      modifiers: new Set(["Shift"]),
      material,
      flexGrow: 2.35,
    });
    const view = render(<ui.KeyboardKeyButton {...state} />);
    const button = screen.getByRole<HTMLButtonElement>("button", { name: "Shift" });
    expect(button.type).toBe("button");
    expect(button.style.flexGrow).toBe("2.35");
    expect(button.getAttribute("data-key-material")).toBe(material);
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(button.classList.contains("keyboard-key")).toBe(true);
    expect(button.className).toContain("bg-[var(--kb-active)]!");
    if (material === "raised") expect(button.className).toContain("-translate-y-px");
    if (material === "glass") expect(button.className).toContain("backdrop-blur-[8px]");
    if (material === "paper") expect(button.className).toContain("repeating-linear-gradient");
    view.rerender(<ui.KeyboardKeyButton {...state} modifiers={new Set()} />);
    expect(button.getAttribute("aria-pressed")).toBe("false");
    expect(button.className).not.toContain("bg-[var(--kb-active)]!");
  },
);

test("普通指针点击不重复激活，辅助点击使用事件时的完整键面文本", () => {
  const state = props({ keyToRender: { label: "a", hint: "合成提示", virtualKey: 0x41 } });
  render(<ui.KeyboardKeyButton {...state} />);
  const button = screen.getByRole("button");
  fireEvent.click(button, { detail: 1 });
  expect(state.onActivate).not.toHaveBeenCalled();
  button.querySelector("[data-key-label]")!.textContent = "A";
  fireEvent.click(button, { detail: 0 });
  expect(state.onActivate).toHaveBeenCalledExactlyOnceWith(state.keyToRender, "A合成提示");
});

test.each([
  { label: "Shift", virtualKey: 0x10, modifier: "Shift" as const },
  { label: "Num Lock", virtualKey: 0x90 },
])("$label 的普通点击仍激活一次", (keyToRender) => {
  const state = props({ keyToRender });
  render(<ui.KeyboardKeyButton {...state} />);
  fireEvent.click(screen.getByRole("button"), { detail: 1 });
  expect(state.onActivate).toHaveBeenCalledExactlyOnceWith(keyToRender, keyToRender.label);
});

test("禁用按钮不激活", () => {
  const state = props({ disabled: true });
  render(<ui.KeyboardKeyButton {...state} />);
  const button = screen.getByRole<HTMLButtonElement>("button");
  expect(button.disabled).toBe(true);
  fireEvent.click(button, { detail: 0 });
  expect(state.onActivate).not.toHaveBeenCalled();
});

test("指针回调保留按钮 currentTarget 和标签 target，并向按下回调传原按键", () => {
  const calls = vi.fn();
  const state = props();
  render(
    <ui.KeyboardKeyButton
      {...state}
      onPointerDown={(event, key) => calls("down", event.currentTarget, event.target, key)}
      onPointerUp={(event) => calls("up", event.currentTarget, event.target)}
      onPointerCancel={(event) => calls("cancel", event.currentTarget, event.target)}
      onPointerLeave={(event) => calls("leave", event.currentTarget)}
    />,
  );
  const button = screen.getByRole("button");
  const label = button.querySelector("[data-key-label]")!;
  fireEvent.pointerDown(label);
  fireEvent.pointerUp(label);
  fireEvent.pointerCancel(label);
  fireEvent.pointerLeave(button);
  expect(calls.mock.calls).toEqual([
    ["down", button, label, state.keyToRender],
    ["up", button, label],
    ["cancel", button, label],
    ["leave", button],
  ]);
});
