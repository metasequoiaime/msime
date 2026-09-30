// @vitest-environment jsdom
import { expect, test } from "vitest";

test("expression page and input panel reuse custom translation settings", () => {
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

  expect(page).toContain(
    'import { CustomTranslationSettingsSection } from "../custom-translation-settings-section";',
  );
  expect(panel).toContain(
    'import { CustomTranslationSettingsSection } from "./custom-translation-settings-section";',
  );
  expect(page).toContain("<CustomTranslationSettingsSection");
  expect(panel).toContain("<CustomTranslationSettingsSection");
  expect(page).not.toContain("<CustomTranslationsSection");
  expect(panel).not.toContain("<CustomTranslationsSection");
});
