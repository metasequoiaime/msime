#pragma once

#include <optional>
#include <string>

namespace msime::windows {
// 候选右键菜单的「朗读」：用 SAPI 5 的本机声音读一个英文单词。只在 Server 的工作线程上调用：在调用线程上初始化 COM，读完才返回（一个词不到一秒），连点时多出来的那几下由调用方的单槽工作线程丢掉。`text` 是 `msime_client_read_aloud_text` 答的那段 UTF-8 英文。读出来时返回空；没有可用的声音、文字不是有效的 UTF-8 或 SAPI 出错时返回给用户看的那句话。SAPI 的声音都装在本机，不联网，见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
std::optional<std::wstring> speak_english(const std::string &text);
} // namespace msime::windows
