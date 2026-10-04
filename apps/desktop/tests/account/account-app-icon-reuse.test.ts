import { expect, test } from "vitest";

test("app icon settings reuse the shared account action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("function AppIconSettingsCard");
  const end = source.indexOf("function SettingsSyncCard", start);
  const card = source.slice(start, end);

  expect(card).toContain("useAccountAction(");
  expect(card).not.toContain("const changeRunning = useRef(false)");
  expect(card).not.toContain("const generation = useRef(0)");
});
