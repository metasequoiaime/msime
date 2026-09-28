// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CredentialTestSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("hides the control when the host cannot test credentials", () => {
  render(
    <CredentialTestSection
      label="测试配置"
      config={{ provider: "synthetic" }}
      available={false}
      onTest={vi.fn()}
    />,
  );

  expect(screen.queryByRole("button", { name: "测试配置" })).toBeNull();
});

test("shows a matching busy result and forwards clicks", () => {
  const onTest = vi.fn();
  const config = { provider: "synthetic", model: "model-a" };
  render(
    <CredentialTestSection
      label="测试语音配置"
      config={config}
      state={{ signature: JSON.stringify(config), busy: true, message: "" }}
      onTest={onTest}
    />,
  );

  const button = screen.getByRole("button", { name: "测试语音配置" }) as HTMLButtonElement;
  expect(button.textContent).toBe("测试中…");
  expect(button.disabled).toBe(true);
  fireEvent.click(button);
  expect(onTest).not.toHaveBeenCalled();
});

test("only shows a result for the current configuration", () => {
  const config = { provider: "synthetic", model: "model-b" };
  render(
    <CredentialTestSection
      label="测试配置"
      config={config}
      state={{
        signature: JSON.stringify({ provider: "synthetic", model: "model-a" }),
        busy: false,
        ok: false,
        message: "旧配置失败",
      }}
      onTest={vi.fn()}
    />,
  );

  expect(screen.queryByText("旧配置失败")).toBeNull();
  expect(screen.getByRole("button", { name: "测试配置" })).toBeTruthy();
});
