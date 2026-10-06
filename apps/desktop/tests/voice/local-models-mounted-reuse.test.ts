import { expect, test } from "vitest";

test("local model manager reuses the shared mounted lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/voice/local-models.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).not.toContain("const mounted = useRef(true)");
});
