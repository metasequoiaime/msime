import { expect, test } from "vitest";

test("input page reuses the shared input scheme selector composition", () => {
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
    'InputSchemeSelectorSection,\n  type InputSchemeSelectorValue,\n} from "./input-scheme-selector-section";',
  );
  expect(shared).toContain("<InputSchemeSelectorSection");
  expect(shared).not.toContain('<Row title="输入方案"');
});
