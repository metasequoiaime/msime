// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsInputDescription } from "@msime/ui";

afterEach(cleanup);

test("renders a settings input description with the shared description style", () => {
  render(<SettingsInputDescription>输入说明</SettingsInputDescription>);

  const description = screen.getByText("输入说明");
  expect(description.tagName).toBe("P");
  expect(description.className).toBe("input-setting-description");
});

test("preserves a status role for pending setting work", () => {
  render(<SettingsInputDescription role="status">正在同步。</SettingsInputDescription>);

  expect(screen.getByRole("status").textContent).toBe("正在同步。");
});
