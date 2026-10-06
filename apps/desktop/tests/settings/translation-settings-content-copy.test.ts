import { expect, test } from "vitest";

test("expression page uses the shared translation settings composition", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(page).toContain(
    'import { TranslationSettingsContent } from "../translation-settings-content";',
  );
  expect(page).toContain("<TranslationSettingsContent");
  expect(page).toContain("createTranslationSettingsBindings");
  expect(page).not.toContain("<CandidateTranslationSettingsSection");
  expect(page).not.toContain("<TranslationProviderSettingsSection");
  expect(page).not.toContain("candidate={{");
});
