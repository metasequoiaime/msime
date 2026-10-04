import { expect, test } from "vitest";

test("update checks reuse the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-update-check.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const requestGeneration = useRef(0)");
});
