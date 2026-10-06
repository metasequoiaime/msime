import { expect, test } from "vitest";

test("custom helpcode schema loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/index.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("function useCustomHelpcodeSchemas");
  const end = source.indexOf("function useHelpcodePacks", start);
  const hook = source.slice(start, end);

  expect(hook).toContain("useAsyncGeneration(reader)");
  expect(hook).not.toContain("let active = true");
});
