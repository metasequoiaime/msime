import { expect, test } from "vitest";

test("cloud clipboard client effects reuse the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function CloudClipboardPanel");
  const end = source.indexOf("export function CloudDictionaryPanel", start);
  const panel = source.slice(start, end);

  expect(panel).toContain("const cloudGeneration = useAsyncGeneration(client)");
  expect(panel).not.toContain("let active = true");
});
