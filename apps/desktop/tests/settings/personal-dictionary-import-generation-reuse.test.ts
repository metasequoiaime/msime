import { expect, test } from "vitest";

test("personal dictionary import reuses the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/personal-dictionary-import-card.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const dictionaryGeneration = useRef(0)");
});
