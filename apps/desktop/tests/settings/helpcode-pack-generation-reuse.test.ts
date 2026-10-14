import { expect, test } from "vitest";

test("helpcode pack loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-helpcode-catalog.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("function useHelpcodePacks");
  const hook = source.slice(start);

  expect(hook).toContain("useAsyncGeneration(catalog, inputPageOpen)");
  expect(hook).not.toContain("let active = true");
});
