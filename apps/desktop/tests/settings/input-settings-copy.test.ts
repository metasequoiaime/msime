import { expect, test } from "vitest";

test("input page and panel reuse shared common settings content", () => {
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
    'import { InputSharedSettingsSection } from "../input-shared-settings-section";',
  );
  expect(panel).toContain(
    'import { InputSharedSettingsSection } from "./input-shared-settings-section";',
  );
  expect(page).toContain("<InputSharedSettingsSection");
  expect(panel).toContain("<InputSharedSettingsSection");
  expect(page).not.toContain("<WordCharacterSection");
  expect(panel).not.toContain("<WordCharacterSection");
  expect(page).not.toContain("<TraditionalChineseOutputSection");
  expect(panel).not.toContain("<TraditionalChineseOutputSection");
});
