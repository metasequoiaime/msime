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
} // namespace msime::tsf
