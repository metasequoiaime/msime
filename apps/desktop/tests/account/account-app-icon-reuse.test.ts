import { expect, test } from "vitest";

test("账号页复用独立图标卡片，卡片状态继续复用账号操作生命周期", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(source).toContain('from "./app-icon-settings-card"');
  expect(source).not.toContain("function AppIconSettingsCard");
  const card = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/app-icon-settings-card.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const state = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/use-app-icon-settings.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(card).toContain("useAppIconSettings(client)");
  expect(state).toContain("useAccountAction(");
  expect(state).not.toContain("const changeRunning = useRef(false)");
  expect(state).not.toContain("const generation = useRef(0)");
});
