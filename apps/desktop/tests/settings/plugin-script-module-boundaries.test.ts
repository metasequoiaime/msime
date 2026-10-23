import { expect, test } from "vitest";

test("脚本规则、授权状态和对话框各有独立模块，调用方按职责依赖", () => {
  const sources = import.meta.glob<string>(
    [
      "../../../../packages/ui/src/settings/plugin-script-model.ts",
      "../../../../packages/ui/src/settings/script-grant-dialog.tsx",
      "../../../../packages/ui/src/settings/use-script-grant-dialog.ts",
      "../../../../packages/ui/src/settings/plugin-script-view.tsx",
      "../../../../packages/ui/src/settings/plugin-list-view.tsx",
      "../../../../packages/ui/src/settings/plugin-library-view.tsx",
      "../../../../packages/ui/src/settings/use-plugins-section.ts",
    ],
    { eager: true, query: "?raw", import: "default" },
  );
  const source = (name: string) => sources[`../../../../packages/ui/src/settings/${name}`];
  const model = source("plugin-script-model.ts");
  const dialog = source("script-grant-dialog.tsx");
  const state = source("use-script-grant-dialog.ts");
  const detail = source("plugin-script-view.tsx");

  expect(model).toBeDefined();
  expect(dialog).toBeDefined();
  expect(state).toBeDefined();
  expect(model).not.toMatch(/from ["'](?:react|.*view|.*dialog|.*controls)["']/);
  expect(dialog).toContain("useScriptGrantDialog(");
  expect(detail).toContain('from "./script-grant-dialog"');
  expect(detail).not.toContain("export function ScriptGrantDialog(");
  expect(detail).not.toContain("export function toggleCapability(");
  for (const name of [
    "plugin-list-view.tsx",
    "plugin-library-view.tsx",
    "use-plugins-section.ts",
  ]) {
    expect(source(name)).toContain('from "./plugin-script-model"');
    expect(source(name)).not.toContain('from "./plugin-script-view"');
  }
});
