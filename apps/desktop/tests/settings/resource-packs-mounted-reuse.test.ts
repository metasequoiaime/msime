import { expect, test } from "vitest";

test("resource packs reuse the shared mounted lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/resource-packs.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).not.toContain("const mounted = useRef(true)");
});
