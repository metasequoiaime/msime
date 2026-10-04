import { expect, test } from "vitest";

test("resource pack progress loading reuses the shared async generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/resource-packs.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(client)");
  expect(source).not.toContain("let cancelled = false");
});
