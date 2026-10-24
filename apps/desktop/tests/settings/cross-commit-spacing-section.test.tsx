// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CrossCommitSpacingSection } from "../../../../packages/ui/src/settings/cross-commit-spacing-section";

afterEach(cleanup);

function show(overrides: Partial<React.ComponentProps<typeof CrossCommitSpacingSection>> = {}) {
  const onChange = vi.fn();
  const onExcludedAppsChange = vi.fn();
  render(
    <CrossCommitSpacingSection
      onChange={onChange}
      onExcludedAppsChange={onExcludedAppsChange}
      {...overrides}
    />,
  );
  return { onChange, onExcludedAppsChange };
}

test("跨次空格默认关闭，修改时只发出独立开关", () => {
  const { onChange, onExcludedAppsChange } = show();
  const toggle = screen.getByLabelText("跨次上屏自动加空格") as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  expect(screen.queryByText("排除应用")).toBeNull();
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(true);
  expect(onExcludedAppsChange).not.toHaveBeenCalled();
});

test("应用排除支持验证、添加及移除", () => {
  const { onExcludedAppsChange } = show({ enabled: true, excludedApps: ["app.example.Code"] });
  const field = screen.getByLabelText("跨次空格排除应用 Bundle ID");
  fireEvent.change(field, { target: { value: "APP.EXAMPLE.CODE" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(screen.getByRole("alert")).toBeTruthy();
  expect(onExcludedAppsChange).not.toHaveBeenCalled();
  fireEvent.change(field, { target: { value: "app.example.Mail" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(onExcludedAppsChange).toHaveBeenLastCalledWith(["app.example.Code", "app.example.Mail"]);
  fireEvent.click(screen.getByRole("button", { name: "移除跨次空格排除应用 app.example.Code" }));
  expect(onExcludedAppsChange).toHaveBeenLastCalledWith([]);
});

test("应用选择器的错误使用跨次空格名单名称", async () => {
  const { onExcludedAppsChange } = show({
    enabled: true,
    pickApplication: vi.fn().mockRejectedValue({ code: "not_application" }),
  });
  fireEvent.click(screen.getByRole("button", { name: "选择应用…" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("跨次空格排除名单"));
  expect(onExcludedAppsChange).not.toHaveBeenCalled();
});
