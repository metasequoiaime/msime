// @vitest-environment jsdom
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
  // The custom glosses editor sits with 显示英文释义 rather than inside a service's group, so hosts place it themselves, but always from the shared binding rather than wiring its props by hand.
  for (const source of [page, panel]) {
    expect(source).toContain("<CustomTranslationsSection {...translation.customGlosses} />");
    expect(source).not.toContain("onFlush={");
  }
  expect(page).not.toContain("<TranslationProviderSettingsSection");
  expect(panel).not.toContain("<TranslationProviderSettingsSection");
});
