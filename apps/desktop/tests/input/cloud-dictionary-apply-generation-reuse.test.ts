import { expect, test } from "vitest";

test("cloud dictionary apply reuses the shared lifecycle generation", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/keyboard/use-cloud-dictionary-apply-panel.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  const panel = source;

  expect(panel).toContain("const lifecycleRevision = useAsyncGeneration(client)");
  expect(panel).not.toContain("const lifecycle = ++lifecycleRevision.current");
  expect(panel).not.toContain("const lifecycleRevision = useRef(0)");
});
