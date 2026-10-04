import { expect, test } from "vitest";

test("clipboard history loading reuses the shared async generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/clipboard-history-section.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const historyGeneration = useAsyncGeneration(");
  expect(source).not.toContain("let active = true");
  expect(source).not.toContain("const historyGeneration = useRef(0)");
});
