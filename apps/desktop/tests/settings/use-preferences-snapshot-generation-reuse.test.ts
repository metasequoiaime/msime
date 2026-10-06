import { expect, test } from "vitest";

test("preference snapshots reuse the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-preferences-snapshot.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(preferences, resetOnClientChange)");
  expect(source).not.toContain("let active = true");
});
