import { expect, test } from "vitest";

test("expression page and input panel share translation settings composition", () => {
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
    'import { TranslationSettingsContent } from "../translation-settings-content";',
  );
  expect(panel).toContain(
    'import { TranslationSettingsContent } from "./translation-settings-content";',
  );
  expect(page).toContain("<TranslationSettingsContent");
  expect(panel).toContain("<TranslationSettingsContent");
  expect(page).toContain("createCandidateTranslationSettings");
  expect(panel).toContain("createCandidateTranslationSettings");
  for (const source of [page, panel]) {
    expect(source).not.toContain("<CandidateTranslationSettingsSection");
    expect(source).not.toContain("<TranslationProviderSettingsSection");
    expect(source).not.toContain("candidate={{");
  }
});
