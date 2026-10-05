import { expect, test } from "vitest";

test("Linux setup page reuses the shared async action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/linux-setup-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
