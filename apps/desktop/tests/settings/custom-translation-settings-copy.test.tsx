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
  // 自定义释义编辑器放在「显示英文释义」旁边，而不在某个服务的组里，所以由宿主自行摆放，但一律通过共享绑定，而不是手工接线它的 props。
  for (const source of [page, panel]) {
    expect(source).toContain("<CustomTranslationsSection {...translation.customGlosses} />");
    expect(source).not.toContain("onFlush={");
  }
  expect(page).not.toContain("<TranslationProviderSettingsSection");
  expect(panel).not.toContain("<TranslationProviderSettingsSection");
});
