import { expect, test } from "vitest";

test("cloud clipboard drafts reuse the shared generation", () => {
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

  expect(panel).toContain("const draftRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const draftRevision = useRef(0)");
});
