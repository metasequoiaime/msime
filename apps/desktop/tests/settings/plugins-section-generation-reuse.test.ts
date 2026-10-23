import { expect, test } from "vitest";

test("插件设置状态独立维护并复用公共代次与操作生命周期", () => {
  const sources = import.meta.glob<string>(
    [
      "../../../../packages/ui/src/settings/plugins-section.tsx",
      "../../../../packages/ui/src/settings/use-plugins-section.ts",
    ],
    {
      eager: true,
      query: "?raw",
      import: "default",
    },
  );
  const section = sources["../../../../packages/ui/src/settings/plugins-section.tsx"];
  const source = sources["../../../../packages/ui/src/settings/use-plugins-section.ts"];

  expect(source).toBeDefined();
  expect(section).toContain("usePluginsSection(");
  expect(section).not.toContain("useAsyncGeneration(");
  expect(section).not.toContain("useAsyncActionRunner(");

  expect(source).toContain("useAsyncGeneration(active, client, mentionsEditable, scriptsShown)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
  expect(source).not.toContain("const generation = ++clientGeneration.current");
  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
