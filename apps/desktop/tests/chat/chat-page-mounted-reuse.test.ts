import { expect, test } from "vitest";

test("对话页委托公共会话，公共会话复用挂载生命周期", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/chat/chat-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const source =
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/chat/use-chat-conversation.ts", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0] ?? "";

  expect(page).toContain("useChatConversation({ client, onLogin, active })");
  expect(source).toContain("useMountedRef()");
  expect(source).not.toContain("const mounted = useRef(true)");
});
