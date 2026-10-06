import { expect, test } from "vitest";

test("AI prompt tests reuse the shared operation generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-ai-assistant.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const testActionOwner = useAsyncGeneration()");
  expect(source).not.toContain("const testActionOwner = useRef(0)");
});
