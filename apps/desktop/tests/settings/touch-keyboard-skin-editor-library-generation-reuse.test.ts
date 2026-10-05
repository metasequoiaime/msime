import { expect, test } from "vitest";

test("touch skin library loading reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(source).toContain("useAsyncActionRunner(setLibraryNotice, undefined, library)");
  expect(source).not.toContain("useAsyncGeneration(library)");
  expect(source).not.toContain("const libraryGeneration = useRef(0)");
});
