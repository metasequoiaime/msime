import { expect, test } from "vitest";

test("handwriting recognition reuses the shared request generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-handwriting-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(source).toContain("const recognitionRevision = useAsyncGeneration()");
  expect(source).not.toContain("const recognitionRevision = useRef(0)");
});
