import { expect, test } from "vitest";

test("candidate skin preview drawing reuses the shared operation generation", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-candidate-skin-publication.ts",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(source).toContain("const drawOwner = useAsyncGeneration()");
  expect(source).not.toContain("const drawOwner = useRef(0)");
});
