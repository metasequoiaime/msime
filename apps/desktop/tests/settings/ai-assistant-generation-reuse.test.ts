import { expect, test } from "vitest";

test("AI assistant requests reuse the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-ai-assistant.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const requestGeneration = useAsyncGeneration(client)");
  expect(source).not.toContain("const requestGeneration = useRef(0)");
});
