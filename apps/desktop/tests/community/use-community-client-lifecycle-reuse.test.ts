import { expect, test } from "vitest";

test("community client lifecycle reuses the shared async action runner", () => {
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

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("useMountedRef()");
  expect(source).not.toContain("useAsyncGeneration(client, ...owners)");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
