import { expect, test } from "vitest";

test("expression page uses the shared Tencent translation binding", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(page).toContain("createTranslationSettingsBindings(");
  expect(page).not.toContain("showMissingCredentialsWarning:");
  expect(page).not.toContain("onSecretIdChange:");
});
