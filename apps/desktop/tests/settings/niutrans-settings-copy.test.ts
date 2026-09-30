import { expect, test } from "vitest";

test("expression page and input panel reuse NiuTrans settings", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/input-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain('import { NiuTransSettingsSection } from "../niutrans-settings-section";');
  expect(panel).toContain('import { NiuTransSettingsSection } from "./niutrans-settings-section";');
  expect(page).toContain("<NiuTransSettingsSection");
  expect(panel).toContain("<NiuTransSettingsSection");
  expect(page).not.toContain("<NiuTransSection");
  expect(panel).not.toContain("<NiuTransSection");
});
