// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CloudPanelHeaderProps = {
  title: string;
  onClose: () => void;
  client?: { beginWindowDrag?(): Promise<void> };
  onBack?: () => void;
};

function pointer(target: Element, type: string, x: number, extra = {}) {
  const event = new Event(type, { bubbles: true });
  Object.assign(event, { pointerId: 1, clientX: x, clientY: 10, button: 0, buttons: 1, ...extra });
  fireEvent(target, event);
}

test("cloud panel header exposes title, close, and optional back actions", () => {
  const Header = (ui as unknown as { CloudPanelHeader: ComponentType<CloudPanelHeaderProps> })
    .CloudPanelHeader;
  expect(Header).toBeDefined();

  const onBack = vi.fn();
  const onClose = vi.fn();
  render(<Header title="导入与导出" onBack={onBack} onClose={onClose} />);

  expect(screen.getByText("导入与导出")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "返回云词库" }));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(onBack).toHaveBeenCalledOnce();
  expect(onClose).toHaveBeenCalledOnce();
});

test("cloud panel header starts one host drag after movement and excludes buttons", () => {
  const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
  const onClose = vi.fn();
  const Header = (ui as unknown as { CloudPanelHeader: ComponentType<CloudPanelHeaderProps> })
    .CloudPanelHeader;
  const view = render(<Header title="云剪贴板" client={{ beginWindowDrag }} onClose={onClose} />);
  const header = view.container.querySelector("header")!;
  pointer(header, "pointerdown", 10);
  pointer(header, "pointermove", 11);
  expect(beginWindowDrag).not.toHaveBeenCalled();
  pointer(header, "pointermove", 13);
  pointer(header, "pointermove", 20);
  expect(beginWindowDrag).toHaveBeenCalledOnce();

  const close = screen.getByRole("button", { name: "关闭" });
  pointer(close, "pointerdown", 10);
  pointer(close, "pointermove", 20);
  expect(beginWindowDrag).toHaveBeenCalledOnce();
  expect(onClose).not.toHaveBeenCalled();
});
