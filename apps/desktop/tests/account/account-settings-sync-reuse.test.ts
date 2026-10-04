import { expect, test } from "vitest";

test("settings sync reuses the shared account action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("function SettingsSyncCard");
  const end = source.indexOf("const hasCloudSettings", start);
  const card = source.slice(start, end);

  expect(card).toContain("useAccountAction(");
  expect(card).not.toContain("const actionBusy = useRef(false)");
  expect(card).not.toContain("const generation = useRef(0)");
});
