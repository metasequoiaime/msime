// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsAboutPage } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/about-settings-section", () => ({
  AboutSettingsSection: () => <section aria-label="关于测试面板" />,
}));

test("exposes the about settings page surface", () => {
  render(<SettingsAboutPage {...({} as any)} />);

  expect(screen.getByRole("region", { name: "关于测试面板" })).toBeTruthy();
});
