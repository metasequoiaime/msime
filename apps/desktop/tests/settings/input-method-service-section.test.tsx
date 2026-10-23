// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputMethodServiceSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("macOS offers one 修复输入源, which brings the bundled source up to date and then registers it", async () => {
  const calls: string[] = [];
  const restart = vi.fn(async () => {
    calls.push("register");
  });
  const install = vi.fn(async () => {
    calls.push("install");
  });
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      installInputSource={install}
    />,
  );

  expect(screen.getByText("修复输入源")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重新注册" })).toBeNull();
  expect(screen.queryByRole("button", { name: "安装 / 更新" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(await screen.findByText("输入源已修复。")).toBeTruthy();
  expect(calls).toEqual(["install", "register"]);
});

test("macOS repair only registers when there is no bundled source to install", async () => {
  const restart = vi.fn(async () => {});
  render(<InputMethodServiceSection visible macos linux={false} restartInputMethod={restart} />);
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(await screen.findByText("输入源已修复。")).toBeTruthy();
  expect(restart).toHaveBeenCalledOnce();
});

test("a failed install still re-registers the installed source, then reports the failure", async () => {
  // 安装失败（随附的 bundle 坏了、复制或登记新版本失败）不能挡住重新登记：原来单独的「重新注册」能修好的情况照样修，安装失败在注册之后报出来。
  const calls: string[] = [];
  const restart = vi.fn(async () => {
    calls.push("register");
  });
  const install = vi.fn(async () => {
    calls.push("install");
    throw { code: "unavailable" };
  });
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      installInputSource={install}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(await screen.findByText(/修复输入源失败/)).toBeTruthy();
  expect(calls).toEqual(["install", "register"]);
});

test("a failed registration fails the repair", async () => {
  const restart = vi.fn(async () => {
    throw { code: "unavailable" };
  });
  const install = vi.fn(async () => {});
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      installInputSource={install}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(await screen.findByText(/修复输入源失败/)).toBeTruthy();
  expect(install).toHaveBeenCalledOnce();
});

test("service section uses Linux restart wording", () => {
  render(<InputMethodServiceSection visible macos={false} linux restartInputMethod={vi.fn()} />);

  expect(
    screen.getByText(
      "重启 IBus 输入法服务；使用 Fcitx5 时重载水杉插件，关闭并重建所有输入会话，不影响其他输入法。",
    ),
  ).toBeTruthy();
  expect(screen.getByText("立即重启输入法服务")).toBeTruthy();
});

test("取消系统修复授权后停止修复并说明取消", async () => {
  const restart = vi.fn(async () => {});
  const install = vi.fn(async () => {
    throw { code: "cancelled" };
  });
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      installInputSource={install}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(
    await screen.findByText("已取消管理员授权，输入源未完成修复。可以重新点击修复。"),
  ).toBeTruthy();
  expect(restart).not.toHaveBeenCalled();
  expect(screen.queryByText("输入源已修复。")).toBeNull();
});

test("修复结束后主动重读安装状态，无需等待窗口重新聚焦", async () => {
  const calls: string[] = [];
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      installInputSource={async () => {
        calls.push("install");
      }}
      restartInputMethod={async () => {
        calls.push("register");
      }}
      onRepairFinished={async () => {
        calls.push("refresh");
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "修复" }));
  expect(await screen.findByText("输入源已修复。")).toBeTruthy();
  expect(calls).toEqual(["install", "register", "refresh"]);
});
