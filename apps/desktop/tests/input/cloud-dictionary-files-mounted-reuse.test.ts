import { expect, test } from "vitest";

test("cloud dictionary files panel reuses the shared mounted lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/keyboard/use-cloud-dictionary-files-panel.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  const panel = source;

  expect(panel).toContain("const mounted = useMountedRef()");
  expect(panel).not.toContain("const mounted = useRef(true)");
});
