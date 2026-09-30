import { expect, test } from "vitest";

test("expression page uses the grouped input language options component", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain(
    'import { InputLanguageOptionsSection } from "../input-language-options-section";',
  );
  expect(page).toContain("<InputLanguageOptionsSection");
  expect(page).toContain("grouped");
  expect(page).not.toContain("<MixedInputSection");
  expect(page).not.toContain("<CandidateEnglishGlossSection");
  expect(page).not.toContain("<EnglishSuggestionsSection");
});

test("input settings share the flat input language options component", () => {
  const shared = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/input-shared-settings-section.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/input-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(shared).toContain('from "./input-language-options-section";');
  expect(shared).toContain("InputLanguageOptionsSection");
  expect(shared).toContain("<InputLanguageOptionsSection");
  expect(panel).not.toContain("<MixedInputSection");
  expect(panel).not.toContain("<CandidateEnglishGlossSection");
  expect(panel).not.toContain("<EnglishSuggestionsSection");
});
