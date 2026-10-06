import { expect, test } from "vitest";

test("account actions reuse shared mounted and generation lifecycles", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-operation.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).toContain("options.allowBusy");
  expect(source).not.toContain("useMountedRef()");
  expect(source).not.toContain("useAsyncGeneration(client, ...owners)");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
