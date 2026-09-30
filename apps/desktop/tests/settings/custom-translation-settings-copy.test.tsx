// @vitest-environment jsdom
import { expect, test } from "vitest";

test("expression page and input panel share the translation provider binding", () => {
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
    'import { TranslationProviderSettingsSection } from "../translation-provider-settings-section";',
  );
  expect(panel).toContain(
    'import { TranslationProviderSettingsSection } from "./translation-provider-settings-section";',
  );
  expect(page).toContain("<TranslationProviderSettingsSection");
  expect(panel).toContain("<TranslationProviderSettingsSection");
  expect(page).not.toContain("<CustomTranslationsSection");
  expect(panel).not.toContain("<CustomTranslationsSection");
});
