import { expect, test } from "vitest";

test("account avatar loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/account/account-avatar.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(url, load)");
  expect(source).not.toContain("let active = true");
});
