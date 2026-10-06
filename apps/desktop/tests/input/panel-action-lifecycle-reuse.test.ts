import { expect, test } from "vitest";

test("panel actions reuse shared mounted and generation hooks", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-panel-action.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).toContain("invalidate");
  expect(source).not.toContain("useMountedRef()");
  expect(source).not.toContain("useAsyncGeneration()");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const revisionRef = useRef(0)");
});
