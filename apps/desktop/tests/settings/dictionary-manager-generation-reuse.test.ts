import { expect, test } from "vitest";

test("dictionary manager reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-dictionary-manager.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const phraseRequestGeneration = useAsyncGeneration(client.dictionary)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
  expect(source).not.toContain("const phraseRequestGeneration = useRef(0)");
});
