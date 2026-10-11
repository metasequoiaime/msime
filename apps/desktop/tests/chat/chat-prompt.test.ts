import { expect, test } from "vitest";
import type { ChatMessage } from "../../../../packages/ui/src/chat/chat-page";
import { CHAT_SYSTEM_PROMPT, chatRequest } from "../../../../packages/ui/src/chat/chat-prompt";

const turns = (count: number): ChatMessage[] =>
  Array.from({ length: count }, (_, index) => ({
    role: index % 2 === 0 ? "user" : "assistant",
    content: `消息${index}`,
  }));

test("puts the system prompt first and keeps the newest thirteen turns", () => {
  const request = chatRequest(turns(20));
  expect(request).toHaveLength(14);
  expect(request[0]).toEqual({ role: "system", content: CHAT_SYSTEM_PROMPT });
  expect(request.slice(1).map((message) => message.content)).toEqual(
    turns(20)
      .slice(-13)
      .map((message) => message.content),
  );
});

test("sends the system prompt even for the first message", () => {
  expect(chatRequest([{ role: "user", content: "shuishan" }])).toEqual([
    { role: "system", content: CHAT_SYSTEM_PROMPT },
    { role: "user", content: "shuishan" },
  ]);
});
