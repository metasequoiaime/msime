// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsVisualPages } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/appearance-settings-section", () => ({
  AppearanceSettingsSection: () => <section aria-label="外观测试面板" />,
}));
vi.mock("../../../../packages/ui/src/settings/skin-settings-section", () => ({
  SkinSettingsSection: () => <section aria-label="皮肤测试面板" />,
}));
vi.mock("../../../../packages/ui/src/settings/floating-toolbar-settings-section", () => ({
  FloatingToolbarSettingsSection: () => <section aria-label="工具栏测试面板" />,
}));

test("composes visual settings pages", () => {
  render(
    <SettingsVisualPages
      appearance={{} as never}
      skin={{} as never}
      floatingToolbar={{} as never}
    />,
  );

  expect(screen.getByRole("region", { name: "外观测试面板" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "皮肤测试面板" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "工具栏测试面板" })).toBeTruthy();
});
