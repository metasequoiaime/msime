import { expect, test } from "vitest";

test("expression page reuses the translation service selector", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const candidate = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/candidate-translation-settings-section.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(candidate).toContain(
    'TranslationServiceSelectorSection,\n  type TranslationProvider,\n} from "./translation-service-selector-section";',
  );
  expect(candidate).toContain("<TranslationServiceSelectorSection");
  expect(page).toContain("<TranslationSettingsContent");
  expect(page).not.toContain('<select\n                  aria-label="候选词翻译服务"');
});
