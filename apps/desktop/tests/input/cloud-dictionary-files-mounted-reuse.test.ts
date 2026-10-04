import { expect, test } from "vitest";

test("cloud dictionary files panel reuses the shared mounted lifecycle hook", () => {
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

  expect(panel).toContain("const mounted = useMountedRef()");
  expect(panel).not.toContain("const mounted = useRef(true)");
});
