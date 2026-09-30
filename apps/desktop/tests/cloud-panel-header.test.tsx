// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CloudPanelHeaderProps = {
  title: string;
  onClose: () => void;
  onBack?: () => void;
};

test("cloud panel header exposes title, close, and optional back actions", () => {
  const Header = (ui as unknown as { CloudPanelHeader: ComponentType<CloudPanelHeaderProps> })
    .CloudPanelHeader;
  expect(Header).toBeDefined();

  const onBack = vi.fn();
  const onClose = vi.fn();
  render(<Header title="导入与导出" onBack={onBack} onClose={onClose} />);

  expect(screen.getByText("导入与导出")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "返回云词典" }));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(onBack).toHaveBeenCalledOnce();
  expect(onClose).toHaveBeenCalledOnce();
});
