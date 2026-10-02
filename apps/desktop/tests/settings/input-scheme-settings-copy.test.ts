import { expect, test } from "vitest";

test("input page uses the shared input scheme composition", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(page).toContain(
    'import { InputSchemeSettingsContent } from "../input-scheme-settings-content";',
  );
  expect(page).toContain("<InputSchemeSettingsContent");
  expect(page).not.toContain("<InputSchemeDetailsSection");
  expect(page).not.toContain("<InputSchemeSelectorSection");
  expect(page).not.toContain("<WubiSection");
});
