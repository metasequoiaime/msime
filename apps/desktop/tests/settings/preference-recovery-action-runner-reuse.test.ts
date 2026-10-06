import { expect, test } from "vitest";

test("preference recovery actions reuse the shared async action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-preference-recovery.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
