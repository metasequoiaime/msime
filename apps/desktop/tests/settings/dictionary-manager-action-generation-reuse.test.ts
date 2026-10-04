import { expect, test } from "vitest";

test("dictionary manager reuses the shared phrase action generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-dictionary-manager.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const phraseActionOwner = useAsyncGeneration()");
  expect(source).not.toContain("const phraseActionOwner = useRef(0)");
});
