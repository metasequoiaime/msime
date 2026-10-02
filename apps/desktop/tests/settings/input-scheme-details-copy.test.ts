import { expect, test } from "vitest";

test("input page delegates scheme-specific controls to the shared scheme content", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/input-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const shared = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/input-scheme-settings-content.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(page).toContain(
    'import { InputSchemeSettingsContent } from "../input-scheme-settings-content";',
  );
  expect(page).toContain("<InputSchemeSettingsContent");
  expect(shared).toContain(
    'import { InputSchemeDetailsSection, type ShuangpinProfile } from "./input-scheme-details-section";',
  );
  expect(shared).toContain("<InputSchemeDetailsSection");
  expect(shared).not.toContain('<Row title="双拼方案"');
  expect(shared).not.toContain('<Row title="五笔方案"');
  expect(shared).not.toContain('<Row\n            title="日语方案"');
});
