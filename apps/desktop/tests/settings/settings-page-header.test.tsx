// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { SettingsPageHeader } from "../../../../packages/ui/src/settings/settings-page-header";

test("root and subpage headings keep their levels and back-button state", () => {
  const onBack = vi.fn();
  const view = render(<SettingsPageHeader title="设置" onBack={onBack} />);
  expect(screen.getByRole("heading", { level: 1, name: "设置" }).id).toBe("page-title");
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(onBack).toHaveBeenCalledOnce();

  view.rerender(<SettingsPageHeader title="子页" onBack={onBack} level={2} backDisabled />);
  expect(screen.getByRole("heading", { level: 2, name: "子页" }).id).toBe("");
  expect((screen.getByRole("button", { name: "返回" }) as HTMLButtonElement).disabled).toBe(true);
});
