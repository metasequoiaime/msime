import { expect, test } from "vitest";

test("input page reuses shared common settings content", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(page).toContain(
    'import { InputSharedSettingsSection } from "../input-shared-settings-section";',
  );
  expect(page).toContain("<InputSharedSettingsSection");
  expect(page).not.toContain("<WordCharacterSection");
  expect(page).not.toContain("<TraditionalChineseOutputSection");
});
