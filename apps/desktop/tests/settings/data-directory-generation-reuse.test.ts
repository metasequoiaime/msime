import { expect, test } from "vitest";

test("data directory actions reuse the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-data-directory.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const generation = useRef(0)");
});
