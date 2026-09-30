import { expect, test } from "vitest";

test("expression page and input panel reuse Tencent translation settings", () => {
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
    'import { TencentTranslationSettingsSection } from "../tencent-translation-settings-section";',
  );
  expect(panel).toContain(
    'import { TencentTranslationSettingsSection } from "./tencent-translation-settings-section";',
  );
  expect(page).toContain("<TencentTranslationSettingsSection");
  expect(panel).toContain("<TencentTranslationSettingsSection");
  expect(page).not.toContain("<TencentTranslationSection");
  expect(panel).not.toContain("<TencentTranslationSection");
  expect(page).not.toContain("<LinuxTencentCredentialsSection");
  expect(panel).not.toContain("<LinuxTencentCredentialsSection");
});
