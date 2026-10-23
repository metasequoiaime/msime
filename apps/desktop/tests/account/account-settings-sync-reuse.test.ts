import { expect, test } from "vitest";

test("账号页复用独立同步卡片，状态复用账号操作生命周期与同一个读取方法", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(source).toContain('from "./settings-sync-card"');
  expect(source).not.toContain("function SettingsSyncCard");
  const card = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/settings-sync-card.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const state = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/use-settings-sync.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(card).toContain("useSettingsSync(client, userId)");
  expect(state).toContain("useAccountAction(");
  expect(state.match(/client\.schema\(\)/g)).toHaveLength(1);
  expect(state.match(/client\.load\(\)/g)).toHaveLength(1);
  expect(state).not.toContain("const actionBusy = useRef(false)");
  expect(state).not.toContain("const generation = useRef(0)");
});
