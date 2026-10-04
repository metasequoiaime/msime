import { expect, test } from "vitest";

test("toolbar CSS loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/skin/use-toolbar-css.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("let active = true");
});
