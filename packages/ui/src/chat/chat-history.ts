import type { ChatMessage } from "./chat-page";
import { utf8ByteLength } from "../core/text";

const MAX_CHAT_MESSAGES = 14;
const MAX_CHAT_HISTORY_BYTES = 48_000;
export const MAX_MESSAGE_BYTES = 16 * 1024;
export function chatMessageByteLength(content: string): number {
  return utf8ByteLength(content);
}

export function boundedHistory(messages: readonly ChatMessage[]): ChatMessage[] {
  const result: ChatMessage[] = [];
  let bytes = 0;
  for (const message of [...messages].reverse()) {
    const nextBytes = bytes + chatMessageByteLength(message.content);
    if (result.length >= MAX_CHAT_MESSAGES || nextBytes >= MAX_CHAT_HISTORY_BYTES) break;
    result.unshift({ role: message.role, content: message.content });
    bytes = nextBytes;
  }
  return result;
}
