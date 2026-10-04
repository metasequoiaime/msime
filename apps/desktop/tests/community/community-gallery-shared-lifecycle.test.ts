import { expect, test } from "vitest";

test("community gallery requests use the shared client lifecycle guard", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-gallery.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const currentClient = clientGeneration.current");
  expect(source).toContain("isCurrent(currentClient)");
  expect(source).not.toContain("listGeneration.current += 1");
});
