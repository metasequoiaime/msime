// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsDictionaryPage } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/dictionary-settings-panel", () => ({
  DictionarySettingsPanel: () => <section aria-label="词典测试面板" />,
}));

test("exposes the dictionary settings page surface", () => {
  render(<SettingsDictionaryPage {...({} as any)} />);

  expect(screen.getByRole("region", { name: "词典测试面板" })).toBeTruthy();
});
