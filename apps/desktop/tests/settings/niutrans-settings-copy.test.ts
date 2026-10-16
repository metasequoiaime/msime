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
  expect(page).not.toContain("<NiuTransSection");
  // 唯一的直接使用方是 HarmonyOS 手机页，它用同一套共用绑定把所选服务的各行收进「更多选项」。
  expect(page.match(/<TranslationProviderSettingsSection/g)).toHaveLength(1);
  // 外层已经整体折叠，腾讯云地域不再套第二层「更多选项」。
  expect(page).toMatch(
    /<TranslationProviderSettingsSection\s+\{\.\.\.providers\}\s+grouped=\{false\}\s+foldRegion=\{false\}\s*\/>/,
  );
});
