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
  // 自定义释义编辑器放在「显示英文释义」旁边，而不在某个服务的组里，所以由宿主自行摆放，但一律通过共享绑定，而不是手工接线它的 props。
  expect(page).toContain("<CustomTranslationsSection {...translation.customGlosses} />");
  expect(page).not.toContain("onFlush={");
  expect(page).not.toContain("<TranslationProviderSettingsSection");
});
