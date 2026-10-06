import { expect, test } from "vitest";

test("touch skin editor reuses the shared mounted lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).toContain("useAsyncGeneration(client, requestId)");
  expect(source).not.toContain("let active = true");
  expect(source).not.toContain("const mounted = useRef(true)");
});
