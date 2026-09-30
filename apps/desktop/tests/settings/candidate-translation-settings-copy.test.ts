import { expect, test } from "vitest";

test("expression page and input panel share candidate translation settings binding", () => {
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
    'import { CandidateTranslationSettingsSection } from "../candidate-translation-settings-section";',
  );
  expect(panel).toContain(
    'import { CandidateTranslationSettingsSection } from "./candidate-translation-settings-section";',
  );
  expect(page).toContain("<CandidateTranslationSettingsSection");
  expect(panel).toContain("<CandidateTranslationSettingsSection");
  for (const source of [page, panel]) {
    expect(source).not.toContain("<CandidateTranslationOptionsSection");
    expect(source).not.toContain("<OnDeviceTranslationNotice");
  }
});
