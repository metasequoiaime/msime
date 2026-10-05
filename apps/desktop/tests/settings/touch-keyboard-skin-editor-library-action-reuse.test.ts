import { expect, test } from "vitest";

test("touch skin editor reuses one library action runner", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];
  const start = source.indexOf("export function TouchKeyboardSkinEditor");
  const editor = source.slice(start);

  expect(editor).toContain("const runLibraryAction =");
  expect(editor.match(/formatError: libraryError/g)).toHaveLength(1);
});
