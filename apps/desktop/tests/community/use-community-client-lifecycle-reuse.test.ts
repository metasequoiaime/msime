import { expect, test } from "vitest";

test("community client lifecycle reuses shared mounted and generation hooks", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-client-lifecycle.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).toContain("useAsyncGeneration(client, ...owners)");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
});
