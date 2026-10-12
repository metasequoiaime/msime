import { expect, test } from "vitest";

test("dictionary discovery reuses the shared async generation guard", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/harmony-phone-dictionary.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const discoveryGeneration = useAsyncGeneration(community)");
  expect(source).not.toContain("let current = true");
});
