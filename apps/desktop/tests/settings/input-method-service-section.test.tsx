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

test("macOS 装在用户目录时提供迁移，系统安装或范围未知时不提供", async () => {
  const restart = vi.fn(async () => {});
  const calls: string[] = [];
  const migrate = vi.fn(async () => {
    calls.push("migrate");
  });
  const refresh = vi.fn(async () => {
    calls.push("refresh");
  });
  const rendered = render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      userOnlyInstall
      migrateToSystem={migrate}
      onRepairFinished={refresh}
    />,
  );
  expect(screen.getByText("迁移到系统目录")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "迁移" }));
  expect(await screen.findByText("已迁移到系统目录。")).toBeTruthy();
  // 迁移结束后重读安装状态，提示和按钮才会随新的安装位置消失。
  expect(calls).toEqual(["migrate", "refresh"]);
  rendered.unmount();

  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={restart}
      migrateToSystem={migrate}
    />,
  );
  expect(screen.queryByText(/安全输入期间/)).toBeNull();
  expect(screen.queryByRole("button", { name: "迁移" })).toBeNull();
});

test("取消迁移的授权时说明输入法仍在原处，并照样重读安装状态", async () => {
  const refresh = vi.fn(async () => {});
  render(
    <InputMethodServiceSection
      visible
      macos
      linux={false}
      restartInputMethod={vi.fn(async () => {})}
      userOnlyInstall
      migrateToSystem={() => Promise.reject({ code: "cancelled" })}
      onRepairFinished={refresh}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "迁移" }));
  expect(await screen.findByText(/已取消管理员授权，输入法仍在原来的位置/)).toBeTruthy();
  expect(refresh).toHaveBeenCalled();
});
