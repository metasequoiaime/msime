import { expect, test, vi } from "vitest";
import { createSettingsPageSelection } from "@msime/ui";

test("adapts shared string page links to typed navigation", () => {
  const selectPage = vi.fn();
  const actions = createSettingsPageSelection({ selectPage });

  actions.onOpenPage("screen-keyboard");

  expect(selectPage).toHaveBeenCalledWith("screen-keyboard");
});
