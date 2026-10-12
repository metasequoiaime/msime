import { expect, test } from "vitest";

test("external theme preview reuses the shared async generation guard", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/skin/external-appearance-preview.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(resolve, key)");
  expect(source).toContain("useAsyncGeneration(scan, key, active, id)");
  expect(source).toContain("generation.current === current");
});
