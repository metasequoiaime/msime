import { expect, test } from "vitest";

test("candidate skin preview loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-candidate-skins.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(source).toContain("useAsyncGeneration(id, load)");
  expect(source).not.toContain("let active = true");
});
