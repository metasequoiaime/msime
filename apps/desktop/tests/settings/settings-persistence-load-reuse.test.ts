import { expect, test } from "vitest";

test("settings persistence loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-settings-persistence.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const end = source.indexOf("\n  async function reload()");
  const start = source.lastIndexOf("  useEffect(() => {", end);
  const loading = source.slice(start, end);

  expect(source).toContain("useAsyncGeneration(client)");
  expect(loading).toContain("generation.current");
  expect(loading).not.toContain("let active = true");
});
