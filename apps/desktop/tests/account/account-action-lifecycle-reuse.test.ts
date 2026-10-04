import { expect, test } from "vitest";

test("account actions reuse shared mounted and generation lifecycles", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-operation.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).toContain("useAsyncGeneration(client, ...owners)");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
});
