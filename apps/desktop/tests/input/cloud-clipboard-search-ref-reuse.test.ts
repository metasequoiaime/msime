import { expect, test } from "vitest";

test("cloud clipboard search reuses the shared latest ref", () => {
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

  expect(panel).toContain("const searchRef = useLatestRef(search)");
  expect(panel).not.toContain('const searchRef = useRef("")');
});
