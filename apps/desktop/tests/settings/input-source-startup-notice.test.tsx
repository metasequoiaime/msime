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
    "水杉输入法已安装：1.0.0。",
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
    expect(onError).toHaveBeenCalledWith("无法打开系统设置，请手动前往 系统设置 > 键盘 > 输入法。"),
  );
});
