import { expect, test } from "vitest";

test("community gallery request sequencing reuses shared generation refs", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-gallery.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const listGeneration = useAsyncGeneration()");
  expect(source).toContain("const detailGeneration = useAsyncGeneration()");
  expect(source).not.toContain("const listGeneration = useRef(0)");
  expect(source).not.toContain("const detailGeneration = useRef(0)");
});
