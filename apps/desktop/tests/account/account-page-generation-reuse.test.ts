import { expect, test } from "vitest";

test("account page bootstrap reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("clientGeneration.current");
  expect(source).not.toContain("let active = true");
});
