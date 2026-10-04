import { expect, test } from "vitest";

test("cloud dictionary files reuse the shared lifecycle generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function CloudDictionaryFilesPanel");
  const end = source.indexOf("export function CloudDictionaryApplyPanel", start);
  const panel = source.slice(start, end);

  expect(panel).toContain("const lifecycleRevision = useAsyncGeneration(client)");
  expect(panel).not.toContain("const current = ++lifecycleRevision.current");
  expect(panel).not.toContain("const lifecycleRevision = useRef(0)");
});
