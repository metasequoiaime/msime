#!/usr/bin/env python3
"""Every AI chat page sends the same system prompt, and sends it at all.

The prompt tells the model where it is: inside 水杉输入法, talking to someone who is usually trying the keyboard, so a bare `shuishan` is pinyin and not a question. Without it the model introduces itself by its vendor's defaults ("我是 MiniMax-M3") and answers like a general chatbot.

The shared settings page is the source (`packages/ui/src/chat/chat-prompt.ts`). Android and iOS have native chat pages that cannot import it, so each carries a copy. This check exists because the copies had already drifted the worst way: the shared AI 对话 page (the HarmonyOS 对话 tab) and the iOS keyboard chat sent no system prompt at all, while the try-keyboard page and Android each had their own one-line version.

Two things are checked: each copy is byte-identical to the source, and each chat page actually puts the prompt in its request, because a constant that nothing sends is the state this replaced.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

SOURCE = ("shared settings page", "packages/ui/src/chat/chat-prompt.ts", r'export const CHAT_SYSTEM_PROMPT =\s*"((?:[^"\\]|\\.)*)";')
COPIES = [
    ("Android", "platforms/android/java/app/msime/android/home/ChatFragment.java", r'private static final String SYSTEM_PROMPT =\s*"((?:[^"\\]|\\.)*)";'),
    ("iOS", "platforms/ios/App/Sources/keyboard/KeyboardChatHistoryPolicy.swift", r'static let systemPrompt = "((?:[^"\\]|\\.)*)"'),
]
# 检查页面确实委托公共会话，且各请求构造者都带上提示词。
SENDERS = [
    ("shared AI 对话 page", "packages/ui/src/chat/chat-page.tsx", "useChatConversation({ client, onLogin, active })"),
    ("shared AI 对话 conversation", "packages/ui/src/chat/use-chat-conversation.ts", "chatRequest(history)"),
    ("shared try-keyboard page", "packages/ui/src/keyboard/try-keyboard-page.tsx", "chatRequest(conversation)"),
    ("shared request builder", "packages/ui/src/chat/chat-prompt.ts", '{ role: "system", content: CHAT_SYSTEM_PROMPT }'),
    ("Android chat page", "platforms/android/java/app/msime/android/home/ChatFragment.java", 'new Turn("system", SYSTEM_PROMPT)'),
    ("iOS request builder", "platforms/ios/App/Sources/keyboard/KeyboardChatHistoryPolicy.swift", '(role: "system", text: systemPrompt)'),
    ("iOS keyboard chat", "platforms/ios/App/Sources/keyboard/KeyboardChatModel.swift", "KeyboardChatHistoryPolicy.request("),
]


def literal(host: str, relative: str, pattern: str) -> str | None:
    path = ROOT / relative
    if not path.exists():
        failures.append(f"{host}: {relative} is gone; this check is pointed at nothing")
        return None
    match = re.search(pattern, path.read_text())
    if not match:
        failures.append(f"{host}: no system prompt literal found in {relative}")
        return None
    return match.group(1)


failures: list[str] = []
source = literal(*SOURCE)
for host, relative, pattern in COPIES:
    copy = literal(host, relative, pattern)
    if source is not None and copy is not None and copy != source:
        failures.append(f"{host}: the system prompt in {relative} differs from {SOURCE[1]}")
for host, relative, needle in SENDERS:
    path = ROOT / relative
    if not path.exists():
        failures.append(f"{host}: {relative} is gone; this check is pointed at nothing")
    elif needle not in path.read_text():
        failures.append(f"{host}: {relative} no longer sends the system prompt (expected `{needle}`)")

if failures:
    for line in failures:
        print(line)
    sys.exit("the AI chat system prompt differs between hosts or is not sent; edit chat-prompt.ts and copy it verbatim")

print(f"chat system prompt: identical in {len(COPIES) + 1} places and sent by {len(SENDERS)} call sites")
