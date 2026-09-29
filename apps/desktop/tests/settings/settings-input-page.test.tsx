// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsInputPage } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/input-settings-panel", () => ({
  InputSettingsPanel: () => <section aria-label="输入测试面板" />,
}));

test("exposes the input settings page surface", () => {
  render(<SettingsInputPage {...({} as any)} />);

  expect(screen.getByRole("region", { name: "输入测试面板" })).toBeTruthy();
});
