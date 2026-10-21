#pragma once

#include <algorithm>
#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace msime::mac {

// 候选释义显示前的整理规则，纯逻辑，不依赖窗口和会话。
//
// 改动前的 macOS 把整条释义原样画出来。这里按 Windows v0.11.1 的
// `server/src/ipc/candidate_translation_policy.h` 补齐两个开关：`show_pos` 关闭时去掉释义
// 开头的 n.、v. 等词性标签，`single_sense` 打开时只保留第一条释义；`first_synonym` 是
// Windows 给离线词典条目用的更进一步截断（取首个逗号前的一个同义词），macOS 的候选快照
// 不带“来自词典”的标记，所以宿主传 `false`。

// 词性标签在释义片段开头时的长度（不含其后的空格），没有标签时为 0。
inline std::size_t part_of_speech_label_length(std::string_view sense) {
    static constexpr std::string_view kLabels[] = {
        "proper n. ", "interj. ", "idiom. ", "pron. ", "prep. ",
        "conj. ",     "adj. ",    "adv. ",   "num. ",  "det. ",
        "aux. ",      "n. ",      "v. "};
    for (const auto label : kLabels)
        if (sense.size() > label.size() && sense.substr(0, label.size()) == label)
            return label.size() - 1;
    return 0;
}

enum class TranslationSegmentKind {
    Text,
    PartOfSpeech,
    Separator,
};

struct TranslationSegment {
    TranslationSegmentKind kind = TranslationSegmentKind::Text;
    std::string text;
};

// 把一条释义拆成释义文字、词性标签和义项分隔符三段，拼接回去与原文一致。
inline std::vector<TranslationSegment> segment_translation_gloss(std::string_view gloss) {
    static constexpr std::string_view kFullwidthSemicolon = "\xEF\xBC\x9B";
    std::vector<TranslationSegment> segments;
    const auto push = [&segments](TranslationSegmentKind kind, std::string_view text) {
        if (text.empty()) return;
        if (!segments.empty() && segments.back().kind == kind)
            segments.back().text.append(text);
        else
            segments.push_back({kind, std::string(text)});
    };
    const auto pushSense = [&push](std::string_view sense) {
        const std::size_t first = (std::min)(sense.find_first_not_of(" \t\r\n"), sense.size());
        push(TranslationSegmentKind::Text, sense.substr(0, first));
        sense.remove_prefix(first);
        const std::size_t label = part_of_speech_label_length(sense);
        push(TranslationSegmentKind::PartOfSpeech, sense.substr(0, label));
        push(TranslationSegmentKind::Text, sense.substr(label));
    };
    std::size_t start = 0;
    while (start < gloss.size()) {
        const std::size_t ascii = gloss.find(';', start);
        const std::size_t fullwidth = gloss.find(kFullwidthSemicolon, start);
        const std::size_t cut = (std::min)(ascii, fullwidth);
        if (cut == std::string_view::npos) {
            pushSense(gloss.substr(start));
            break;
        }
        pushSense(gloss.substr(start, cut - start));
        const std::size_t width = cut == fullwidth ? kFullwidthSemicolon.size() : 1;
        push(TranslationSegmentKind::Separator, gloss.substr(cut, width));
        start = cut + width;
    }
    return segments;
}

// 整理一条（单目标语言的）释义。只改显示，不改缓存和 Ctrl+Enter 的义项。
inline std::string format_translation_gloss_for_display(std::string_view gloss, bool show_pos,
                                                       bool single_sense = false,
                                                       bool first_synonym = false) {
    if (first_synonym) {
        int depth = 0;
        for (std::size_t i = 0; i < gloss.size(); ++i) {
            const char ch = gloss[i];
            if (ch == '(')
                ++depth;
            else if (ch == ')' && depth > 0)
                --depth;
            else if (ch == ';')
                break;
            else if (ch == ',' && depth == 0) {
                gloss = gloss.substr(0, i);
                break;
            }
        }
    }
    if (show_pos && !single_sense) return std::string(gloss);

    std::string displayed;
    bool after_label = false;
    for (const auto &segment : segment_translation_gloss(gloss)) {
        if (single_sense && segment.kind == TranslationSegmentKind::Separator) break;
        if (!show_pos && segment.kind == TranslationSegmentKind::PartOfSpeech) {
            after_label = true;
            continue;
        }
        std::string_view text = segment.text;
        if (after_label && segment.kind == TranslationSegmentKind::Text)
            text.remove_prefix((std::min)(text.find_first_not_of(" \t\r\n"), text.size()));
        displayed.append(text);
        after_label = false;
    }
    if (single_sense)
        displayed.erase((std::min)(displayed.find_last_not_of(" \t\r\n") + 1, displayed.size()));
    return displayed;
}

// macOS 的释义按目标语言分行（"\n"），每行各自套用上面的规则再拼回去；换行不是义项分隔符，
// 打开 single_sense 时后面几行不该跟着第一条释义一起丢掉。
inline std::string format_translation_for_display(std::string_view gloss, bool show_pos,
                                                  bool single_sense) {
    std::string displayed;
    std::size_t start = 0;
    while (start <= gloss.size()) {
        const std::size_t newline = gloss.find('\n', start);
        const std::string_view line =
            newline == std::string_view::npos ? gloss.substr(start) : gloss.substr(start, newline - start);
        displayed += format_translation_gloss_for_display(line, show_pos, single_sense);
        if (newline == std::string_view::npos) break;
        displayed += '\n';
        start = newline + 1;
    }
    return displayed;
}

} // namespace msime::mac
