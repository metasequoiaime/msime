// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ScreenKeyboardLaunchSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("opens the screen keyboard and exposes its adjustable preview", () => {
  const onOpen = vi.fn();
  render(
    <ScreenKeyboardLaunchSection
      openScreenKeyboard={onOpen}
      theme="light"
      skin="forest"
      keySpacingTenths={60}
      rowSpacingTenths={70}
      heightAdjustment={0}
      onPointerDown={vi.fn()}
      onPointerMove={vi.fn()}
      onPointerUp={vi.fn()}
      onPointerCancel={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "打开" }));
  expect(onOpen).toHaveBeenCalledOnce();
  expect(screen.getByLabelText("屏幕键盘预览")).toBeTruthy();
  expect(screen.getByLabelText("拖动预览调整键盘间距")).toBeTruthy();
});

test("disables the launcher when the host has no open command", () => {
  render(
    <ScreenKeyboardLaunchSection
      theme="dark"
      skin="forest"
      keySpacingTenths={60}
      rowSpacingTenths={70}
      heightAdjustment={0}
      onPointerDown={vi.fn()}
      onPointerMove={vi.fn()}
      onPointerUp={vi.fn()}
      onPointerCancel={vi.fn()}
    />,
  );

  expect((screen.getByRole("button", { name: "打开" }) as HTMLButtonElement).disabled).toBe(true);
});
