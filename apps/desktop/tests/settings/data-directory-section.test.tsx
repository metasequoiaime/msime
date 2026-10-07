// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DataDirectorySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("data directory section reports a directory selection request", () => {
  const onChoose = vi.fn();
  render(
    <DataDirectorySection
      visible
      linux={false}
      dataDirectory={{ path: "/Users/example/Library/Application Support/MSIME", isDefault: true }}
      busy={false}
      result=""
      onChoose={onChoose}
    />,
  );

  expect(screen.getByText("数据目录")).toBeTruthy();
  expect(screen.getByText("/Users/example/Library/Application Support/MSIME")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  expect(onChoose).toHaveBeenCalledOnce();
});

test("data directory section shows Linux credential note and status", () => {
  render(
    <DataDirectorySection
      visible
      linux
      dataDirectory={{ path: "/home/example/.local/share/msime", isDefault: false }}
      busy
      result="数据已移动。设置窗口即将关闭。"
      onChoose={vi.fn()}
    />,
  );

  expect(screen.getByText(/输入法入口配置和在线服务/)).toBeTruthy();
  expect((screen.getByRole("button", { name: "正在移动…" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(screen.getByRole("status").textContent).toContain("数据已移动");
});

test("describes the initial directory read while it is busy", () => {
  const onChoose = vi.fn();
  render(<DataDirectorySection visible linux={false} busy result="" onChoose={onChoose} />);

  const button = screen.getByRole("button", { name: "正在读取…" }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.click(button);
  expect(onChoose).not.toHaveBeenCalled();
});
