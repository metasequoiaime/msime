import { expect, test } from "vitest";

test("handwriting candidate submissions reuse the shared action generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-handwriting-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(source).toContain("const submissionRevision = useAsyncGeneration()");
  expect(source).not.toContain("const submissionRevision = useRef(0)");
});
