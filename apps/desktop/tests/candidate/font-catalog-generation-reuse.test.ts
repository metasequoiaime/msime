import { expect, test } from "vitest";

test("font catalog loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/candidate/font-catalog.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(read, revision, requested)");
  expect(source).not.toContain("let active = true");
});
