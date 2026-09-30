import { expect, test } from "vitest";

test("expression page and input panel reuse the translation service selector", () => {
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
    'import { TranslationServiceSelectorSection } from "../translation-service-selector-section";',
  );
  expect(page).toContain("<TranslationServiceSelectorSection");
  expect(panel).toContain(
    'import {\n  TranslationServiceSelectorSection,\n  type TranslationProvider,\n} from "./translation-service-selector-section";',
  );
  expect(panel).toContain("<TranslationServiceSelectorSection");
  expect(page).not.toContain('<select\n                  aria-label="候选词翻译服务"');
});
