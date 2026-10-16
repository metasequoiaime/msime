import { expect, test } from "vitest";

test("公共对话回复复用客户端代次生命周期", () => {
  const source =
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/chat/use-chat-conversation.ts", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0] ?? "";

  expect(source).toContain("const generation = useAsyncGeneration(client)");
  expect(source).not.toContain("const generation = useRef(0)");
});
