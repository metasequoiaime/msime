import { expect, test } from "vitest";

test("dictionary manifest loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/dictionary-manifest-card.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(read)");
  expect(source).not.toContain("let active = true");
});
