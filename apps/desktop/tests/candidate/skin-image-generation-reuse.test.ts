import { expect, test } from "vitest";

test("skin image loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/skin/skin-image.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(read, id, relative, revision)");
  expect(source).not.toContain("let active = true");
});
