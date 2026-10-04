import { expect, test } from "vitest";

test("candidate font resolution reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/candidate/resolved-candidate-fonts.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(request)");
  expect(source).not.toContain("let active = true");
});
