// @vitest-environment jsdom
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
  // 设置页不再提供自定义候选释义编辑器；Engine 仍读取用户目录里的 custom_translations.txt。
  expect(page).not.toContain("CustomTranslationsSection");
  expect(page).not.toContain("onFlush={");
  expect(page).not.toContain("<TranslationProviderSettingsSection");
});
