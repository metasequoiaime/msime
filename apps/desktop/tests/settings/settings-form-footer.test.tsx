// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsFormFooter, type Preferences } from "@msime/ui";

test("explains the invalid candidate fonts that hold the automatic save back", () => {
  const draft = { candidate_font_family: "" } as Preferences;

  render(
    <SettingsFormFooter
      draft={draft}
      busy={false}
      saveState="idle"
      saveError=""
      showRestoreDefaults={false}
      onRestoreDefaults={vi.fn()}
      onRetry={vi.fn()}
    />,
  );

  expect(screen.getByRole("alert").textContent).toContain("名称不能为空");
  expect(screen.queryByRole("button")).toBeNull();
});
