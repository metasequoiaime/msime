// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputSourceStartupNotice, type InputSourceStartupStatus } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const installed: InputSourceStartupStatus = {
  action: "installed",
  enabled: true,
  bundled_version: "1.0.0",
  installed_version: "1.0.0",
};

test("shows the installation result and can be dismissed", () => {
  const onDismiss = vi.fn();
  render(
    <InputSourceStartupNotice
      status={installed}
      onOpenSettings={vi.fn()}
      onDismiss={onDismiss}
      onError={vi.fn()}
    />,
  );

  expect(screen.getByRole("status", { name: "水杉输入法安装状态" }).textContent).toContain(
    "水杉输入法 1.0.0 已安装",
  );
  fireEvent.click(screen.getByRole("button", { name: "知道了" }));
  expect(onDismiss).toHaveBeenCalledOnce();
});

test("reports a failure when opening keyboard settings fails", async () => {
  const onError = vi.fn();
  render(
    <InputSourceStartupNotice
      status={{ ...installed, enabled: false }}
      onOpenSettings={() => Promise.reject(new Error("settings unavailable"))}
      onDismiss={vi.fn()}
      onError={onError}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "打开键盘设置" }));
  await vi.waitFor(() =>
    expect(onError).toHaveBeenCalledWith(
      "无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。",
    ),
  );
});

test("does not ask for adding the source after a failed install", () => {
  render(
    <InputSourceStartupNotice
      status={{ ...installed, action: "failed", enabled: false }}
      onOpenSettings={vi.fn()}
      onDismiss={vi.fn()}
      onError={vi.fn()}
    />,
  );

  const banner = screen.getByRole("alert", { name: "水杉输入法安装状态" });
  expect(banner.textContent).toContain("水杉输入法没能自动安装或更新");
  // 「安装 / 更新」按钮在「维护与诊断」页，提示要把用户带到那里。
  expect(banner.textContent).toContain("「维护与诊断」页的「输入法服务」");
  expect(banner.textContent).not.toContain("简体中文");
  expect(screen.queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("points a missing install at the maintenance page", () => {
  render(
    <InputSourceStartupNotice
      status={{ ...installed, action: "not_installed", enabled: false }}
      onOpenSettings={vi.fn()}
      onDismiss={vi.fn()}
      onError={vi.fn()}
    />,
  );

  expect(screen.getByRole("status", { name: "水杉输入法安装状态" }).textContent).toContain(
    "请在「维护与诊断」页的「输入法服务」中点「安装 / 更新」。",
  );
});
