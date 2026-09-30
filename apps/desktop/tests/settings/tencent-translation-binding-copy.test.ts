import { expect, test } from "vitest";

test("expression page and input panel use the shared Tencent translation binding", () => {
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

  for (const source of [page, panel]) {
    expect(source).toContain("createTranslationProviderSettings(");
    expect(source).not.toContain("showMissingCredentialsWarning:");
    expect(source).not.toContain("onSecretIdChange:");
  }
});
