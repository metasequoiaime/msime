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
  // 唯一的直接使用者是 HarmonyOS 手机页面，它用同一套共享绑定把所选服务的行收进「更多选项」。
  expect(page.match(/<TranslationProviderSettingsSection/g)).toHaveLength(1);
  expect(page).toContain("<TranslationProviderSettingsSection {...providers} grouped={false} />");
});
