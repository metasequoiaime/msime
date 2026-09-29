import { expect, test } from "vitest";
import { defaultNavigation as sectionDefaultNavigation } from "../../../../packages/ui/src/settings/navigation-section";
import { defaultNavigation as optionsDefaultNavigation } from "../../../../packages/ui/src/settings/settings-options";

test("settings options re-export the navigation section default", () => {
  expect(optionsDefaultNavigation).toBe(sectionDefaultNavigation);
});
