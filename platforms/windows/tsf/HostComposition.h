#pragma once
#include "../common/InputSchemeTraits.h"
#include "EngineSessionAdapter.h"
#include "HostKoreanKey.h"
#include <string>

namespace msime::tsf {
// Printable ASCII, letters included: the character a key that ends a host-composed composition puts in the document right after it, when nothing took its place.
constexpr bool is_host_text_key(wchar_t wch) { return wch >= 0x20 && wch <= 0x7E; }

// What ending the host session's composition for a key left to write.
struct HostCompositionEnd {
    // The UTF-8 text the host session committed, empty when it committed nothing.
    std::string commit;
    // The host had already let go of the composition the document still shows (focus or a scheme change ended it), so the document's composition is ended first and keeps that text.
    bool hostLetGo = true;
    // The key's own character goes in after the commit.
    bool keyFollows = false;
};

// Ends the host session's composition for a key that does not belong to it (KoreanKeyAction::CommitWithText and CommitAndPass).
//
// Zhuyin ends its conversion with an ASCII punctuation key through the Chinese table, the way the Server's session takes the same key: the host session commits the conversion with the full-width mark, or with the key's own character when the table has none, so the key adds nothing after it. Every other key, and a punctuation key the table did not answer with a commit, finishes the composition and the key's printable character follows it as itself, so a letter typed with Shift lands after the Zhuyin conversion it ended rather than being dropped.
template<class Host> HostCompositionEnd EndHostComposition(Host &host, int scheme, wchar_t wch, std::string *error) {
    HostCompositionEnd end;
    end.keyFollows = is_host_text_key(wch);
    std::string raw;
    EngineResult result;
    if (is_korean_text_key(wch) && wch != L' ' && !(wch >= L'0' && wch <= L'9') &&
        msime::windows::scheme::UsesChinesePunctuation(scheme)) {
        const bool held = host.view(&raw, error) && EngineSessionAdapter::parse_result(raw, &result, error) &&
                          !result.view.editing_text.empty();
        raw.clear();
        result = EngineResult{};
        if (host.punctuation(static_cast<uint8_t>(wch), &raw, error) &&
            EngineSessionAdapter::parse_result(raw, &result, error) && result.has_commit && !result.commit.empty()) {
            end.commit = std::move(result.commit);
            end.hostLetGo = !held;
            end.keyFollows = false;
            return end;
        }
        raw.clear();
        result = EngineResult{};
    }
    if (host.command(MSIME_FINISH_COMPOSITION, &raw, error) && EngineSessionAdapter::parse_result(raw, &result, error) &&
        result.has_commit)
        end.commit = std::move(result.commit);
    end.hostLetGo = end.commit.empty();
    return end;
}

// The first Escape on a word whose cancel shows its raw keys again (scheme::CancelRestoresRaw): one MSIME_CANCEL, and true when the host session is still composing after it, so the TIP draws those keys and keeps the composition, as the Server keeps it in its own session. False without sending anything when the scheme discards on Escape or nothing composes, and false when that cancel left nothing, which is the Escape with the raw keys already showing; the caller then discards the composition as for any Escape.
//
// 整句改字时的第一次 Escape 也是这样：一次 MSIME_CANCEL 只退出改字，拼音继续组字（Server 的会话同样只发一次）。
template<class Host> bool RestoreHostRawOnEscape(Host &host, std::string *error) {
    std::string raw;
    EngineResult current;
    if (!host.view(&raw, error) || !EngineSessionAdapter::parse_result(raw, &current, error) ||
        !(msime::windows::scheme::CancelRestoresRaw(static_cast<int>(current.view.scheme)) ||
          !current.view.conversion.empty()) ||
        current.view.editing_text.empty())
        return false;
    raw.clear();
    EngineResult result;
    return host.command(MSIME_CANCEL, &raw, error) && EngineSessionAdapter::parse_result(raw, &result, error) &&
           !result.has_commit && !result.view.editing_text.empty();
}
// 整句改字的方向键是否适用，与 Server 的 sentence_edit_command 同一条件：全拼、双拼，不在本地模式和专用英文里。两边的会话必须对同一个键发同一条命令。
inline bool HostEditsSentence(const EngineView &view) {
    return msime::windows::scheme::EditsSentence(static_cast<int>(view.scheme)) && view.local_mode == "none" &&
           !view.dedicated_english;
}

// 宿主会话的当前视图；读不到时为空。
template<class Host> bool HostView(Host &host, EngineResult *view) {
    std::string raw, error;
    return host.view(&raw, &error) && EngineSessionAdapter::parse_result(raw, view, &error);
}

// 宿主会话正在整句改字。
template<class Host> bool HostConversionActive(Host &host) {
    EngineResult current;
    return HostView(host, &current) && !current.view.conversion.empty();
}

// 把按 Unicode 标量计的位置换成 UTF-8 字节偏移；超出时取末尾。
inline std::size_t Utf8ScalarOffset(const std::string &text, std::size_t scalars) {
    std::size_t offset = 0;
    while (offset < text.size() && scalars > 0) {
        ++offset;
        while (offset < text.size() && (static_cast<unsigned char>(text[offset]) & 0xC0) == 0x80) ++offset;
        --scalars;
    }
    return offset;
}
} // namespace msime::tsf
