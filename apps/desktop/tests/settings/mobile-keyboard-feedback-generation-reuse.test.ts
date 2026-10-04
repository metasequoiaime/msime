import { expect, test } from "vitest";

test("mobile keyboard feedback reuses the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/use-mobile-keyboard-feedback.ts",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];

  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("const generation = useRef(0)");
});
