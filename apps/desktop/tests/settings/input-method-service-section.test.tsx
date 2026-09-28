// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputMethodServiceSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("service section exposes restart and macOS install actions", () => {
  const restart = vi.fn(async () => {});
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

  expect(
    screen.getByText("重新注册并启用已安装的水杉输入源；当前输入法进程继续按系统生命周期运行。"),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "重新注册" }));
  fireEvent.click(screen.getByRole("button", { name: "安装 / 更新" }));
  expect(restart).toHaveBeenCalledOnce();
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
