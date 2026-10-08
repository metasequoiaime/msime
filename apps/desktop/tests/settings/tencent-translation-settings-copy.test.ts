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
  expect(page).not.toContain("<TencentTranslationSection");
  expect(page).not.toContain("<LinuxTencentCredentialsSection");
  // 唯一的直接使用方是 HarmonyOS 手机页面，它用同一套共享绑定把所选服务的行折叠到更多选项下。
  expect(page.match(/<TranslationProviderSettingsSection/g)).toHaveLength(1);
  expect(page).toContain("<TranslationProviderSettingsSection {...providers} grouped={false} />");
});
