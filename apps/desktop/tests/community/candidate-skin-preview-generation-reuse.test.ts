import { expect, test } from "vitest";

test("candidate skin preview drawing reuses the shared operation generation", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/candidate-skin-publish-dialog.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(source).toContain("const drawOwner = useAsyncGeneration()");
  expect(source).not.toContain("const drawOwner = useRef(0)");
});
