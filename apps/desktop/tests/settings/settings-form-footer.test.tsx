// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsFormFooter, type Preferences } from "@msime/ui";

test("blocks saving and explains invalid candidate fonts", () => {
  const draft = {
    candidate_fallback_fonts: Array.from({ length: 33 }, () => "font"),
  } as Preferences;

  render(
    <form>
      <SettingsFormFooter
        draft={draft}
        busy={false}
        dirty
        showRestoreDefaults={false}
        onRestoreDefaults={vi.fn()}
      />
    </form>,
  );

  expect(screen.getByRole("alert").textContent).toContain("补充字体最多 32 项");
  expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(true);
});
