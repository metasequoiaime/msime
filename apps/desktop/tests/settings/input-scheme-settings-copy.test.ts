import { expect, test } from "vitest";

test("input page and panel share input scheme composition", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
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

  expect(page).toContain(
    'import { InputSchemeSettingsContent } from "../input-scheme-settings-content";',
  );
  expect(panel).toContain(
    'import { InputSchemeSettingsContent } from "./input-scheme-settings-content";',
  );
  expect(page).toContain("<InputSchemeSettingsContent");
  expect(panel).toContain("<InputSchemeSettingsContent");
  for (const source of [page, panel]) {
    expect(source).not.toContain("<InputSchemeDetailsSection");
    expect(source).not.toContain("<InputSchemeSelectorSection");
    expect(source).not.toContain("<WubiSection");
  }
});
