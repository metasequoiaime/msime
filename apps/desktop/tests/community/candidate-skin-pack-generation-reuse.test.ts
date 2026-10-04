import { expect, test } from "vitest";

test("candidate skin pack preview reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/candidate-skin-publish-dialog.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(source).toContain("useAsyncGeneration(client, skinId, visibility, packRevision)");
  expect(source).not.toContain("const packGeneration = useRef(0)");
});
