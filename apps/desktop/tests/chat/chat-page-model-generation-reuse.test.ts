import { expect, test } from "vitest";

test("chat model loading reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/chat/chat-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(client)");
  expect(source).not.toContain("const modelGeneration = useRef(0)");
});
