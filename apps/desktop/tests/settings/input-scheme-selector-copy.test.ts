import { expect, test } from "vitest";

test("input page reuses the shared input scheme selector", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain(
    'import { InputSchemeSelectorSection } from "../input-scheme-selector-section";',
  );
  expect(page).toContain("<InputSchemeSelectorSection");
  expect(page).toContain("grouped");
  expect(page).not.toContain('<Row title="输入方案"');
});
