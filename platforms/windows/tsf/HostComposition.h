#pragma once
#include "../common/InputSchemeTraits.h"
#include "EngineSessionAdapter.h"
#include "HostKoreanKey.h"
#include <optional>
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

// 行内「原始按键」样式从 TIP 自己的宿主会话画的文字（UTF-8，不含已选词段）。全拼普通组字时宿主视图的 preedit 是引擎为显示按音节加了隔音符的拼写（`pin'yin`，#9406 起），原始样式要的是打出来的按键，所以画 editing_text，与 macOS、Linux 的原始样式一致；候选窗和「拼音分词」样式仍画 preedit。其他情况照旧画 preedit：双拼的 preedit 随「双拼预编辑」偏好，韩文、注音、越南文、藏文的 preedit 是正在写的文字而 editing_text 是按键，本地模式和专用英文的 preedit 本来就不加隔音符。
inline const std::string &HostRawStyleText(const EngineView &view) {
    return view.scheme == static_cast<uint32_t>(msime::windows::scheme::Quanpin) && view.local_mode == "none" &&
                   !view.dedicated_english
               ? view.editing_text
               : view.preedit;
}

// 宿主会话的当前视图；读不到时为空。
template<class Host> bool HostView(Host &host, EngineResult *view) {
    std::string raw, error;
    return host.view(&raw, &error) && EngineSessionAdapter::parse_result(raw, view, &error);
}

// 高亮所在候选在引擎里的下标，没有高亮行时为空。左右键被共享 runtime 换成移动候选（偏好 `navigation.left_right`：首选上 → 到了输入串末尾、或者高亮不在首选）时预编辑一个字都没变，只有高亮动了；TIP 靠这里分辨出这次按键仍需重画候选窗。
inline std::optional<std::size_t> HighlightedCandidateIndex(const EngineView &view) {
    for (const auto &candidate : view.candidates)
        if (candidate.highlighted) return candidate.index;
    return std::nullopt;
}

// 宿主会话执行一条光标或候选命令之后，行内组字和候选窗要不要按引擎视图整个重画：高亮换了一个候选（左右键被换成移动候选，或者 Home 回到首选），或者整句改字在进行、刚开始或刚结束（预编辑在拼音和改好的句子之间换，候选也换）。都不是时只有拼音光标动了，只移预编辑里的光标就够了。
inline bool HostCompositionNeedsRedraw(const EngineView &before, const EngineView &after) {
    return HighlightedCandidateIndex(before) != HighlightedCandidateIndex(after) || !before.conversion.empty() ||
           !after.conversion.empty();
}

// Home 键发给宿主会话的命令。候选窗显示（视图里有候选）时是两段式的 MSIME_FIRST_CANDIDATE_OR_HOME：高亮不在首选时回到首选，已在首选时把光标移到输入串开头。没有候选时发 MSIME_FIRST_CANDIDATE，什么也不动：Server 的会话只在有候选时接 Home（ReplyComposer::configured_key），两边的光标才不会分开。
inline uint32_t HostHomeCommand(const EngineView &view) {
    return view.candidates.empty() ? static_cast<uint32_t>(MSIME_FIRST_CANDIDATE)
                                   : static_cast<uint32_t>(MSIME_FIRST_CANDIDATE_OR_HOME);
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
