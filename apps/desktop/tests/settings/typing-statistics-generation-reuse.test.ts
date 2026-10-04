import { expect, test } from "vitest";

test("typing statistics reuses shared mounted and generation lifecycles", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/typing-statistics.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useMountedRef(");
  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
});
