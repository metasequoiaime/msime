import { expect, test } from "vitest";

test("community skin library loading reuses the shared async generation guard", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-skins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(harmony, browsing, localSkinLibrary)");
  expect(source).not.toContain("let current = true");
});
