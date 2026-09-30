// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsFormFooter, type Preferences } from "@msime/ui";

test("explains the invalid candidate fonts that hold the automatic save back", () => {
  const draft = {
    candidate_fallback_fonts: Array.from({ length: 33 }, () => "font"),
  } as Preferences;

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

  expect(screen.getByRole("alert").textContent).toContain("补充字体最多 32 项");
  expect(screen.queryByRole("button")).toBeNull();
});
