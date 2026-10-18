import { expect, test } from "vitest";

test("云剪贴板客户端代次复用公共异步生命周期", () => {
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-cloud-clipboard-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(panel).toContain("const cloudGeneration = useAsyncGeneration(client)");
  expect(panel).not.toContain("let active = true");
});
