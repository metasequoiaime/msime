import { expect, test } from "vitest";

test("resource detail loading reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("clientGeneration.current");
  expect(source).not.toContain("let active = true");
});
