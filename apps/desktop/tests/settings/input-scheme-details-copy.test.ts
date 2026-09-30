import { expect, test } from "vitest";

test("input page delegates scheme-specific controls to the shared details section", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain(
    'import { InputSchemeDetailsSection } from "../input-scheme-details-section";',
  );
  expect(page).toContain("<InputSchemeDetailsSection");
  expect(page).toContain("grouped");
  expect(page).not.toContain('<Row title="双拼方案"');
  expect(page).not.toContain('<Row title="五笔方案"');
  expect(page).not.toContain('<Row\n            title="日语方案"');
});
