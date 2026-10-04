import { expect, test } from "vitest";

test("chat reply requests reuse the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/chat/chat-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const generation = useAsyncGeneration(client)");
  expect(source).not.toContain("const generation = useRef(0)");
});
