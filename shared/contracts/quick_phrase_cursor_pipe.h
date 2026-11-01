#pragma once
#include "windows_ipc.h"
#include <optional>

// 已协商的上屏定位元数据；正文仍占用原来的完整 UTF-16 数组，不增加包长或截短正文。
// 最高位表示扩展，低 8 位是原消息类型，中间 16 位是正文尾段的 UTF-16 单元数。
// 第 24..30 位保留为零。只允许真正提交文字的消息携带，不包装导航或配置帧。
namespace FanyImeQuickPhraseCursorPipe {
constexpr std::uint32_t Flag = 0x80000000u;
constexpr std::uint32_t Reserved = 0x7f000000u;

constexpr bool CommitType(std::uint32_t type, bool worker) {
    return worker ? type == FanyImeWorkerReplyType::CommitCurCandidate ||
                        type == FanyImeWorkerReplyType::CommitCandidateAndContinue
                  : type == FanyImeReplyType::Normal || type == FanyImeReplyType::CommitExactText;
}

constexpr bool HasCursor(std::uint32_t type) { return (type & Flag) != 0; }

constexpr std::optional<std::uint32_t> Encode(std::uint32_t type, std::size_t suffixUnits, bool worker) {
    if (!CommitType(type, worker) ||
        suffixUnits > FanyImePipeLimits::CandidateTextMaxLength)
        return std::nullopt;
    return Flag | (static_cast<std::uint32_t>(suffixUnits) << 8) | type;
}

constexpr std::optional<std::size_t> SuffixUnits(std::uint32_t type, bool worker) {
    if (!HasCursor(type) || (type & Reserved) != 0 || !CommitType(type & 0xffu, worker))
        return std::nullopt;
    const auto units = (type >> 8) & 0xffffu;
    if (units > FanyImePipeLimits::CandidateTextMaxLength) return std::nullopt;
    return static_cast<std::size_t>(units);
}

// 非扩展帧原样交给既有校验；损坏的扩展不能被去掉标志后当成普通提交。
constexpr std::optional<std::uint32_t> MessageType(std::uint32_t type, bool worker) {
    if (!HasCursor(type)) return type;
    if (!SuffixUnits(type, worker)) return std::nullopt;
    return type & 0xffu;
}

// 单元数只有在有效、完整的正文中才有意义，不允许定位到代理项对内部。
template<class Char>
bool ValidText(const Char *text, std::size_t capacity, std::size_t suffixUnits) {
    if (!text || suffixUnits > FanyImePipeLimits::CandidateTextMaxLength) return false;
    std::size_t length = 0;
    while (length < capacity && text[length] != 0) ++length;
    if (length == capacity || suffixUnits > length) return false;
    const auto target = length - suffixUnits;
    for (std::size_t i = 0; i < length; ++i) {
        const auto unit = static_cast<std::uint32_t>(text[i]);
        if (unit >= 0xd800 && unit <= 0xdbff) {
            if (++i == length || i == target) return false;
            const auto low = static_cast<std::uint32_t>(text[i]);
            if (low < 0xdc00 || low > 0xdfff) return false;
        } else if (unit >= 0xdc00 && unit <= 0xdfff) return false;
        else if (unit > 0xffff) return false;
    }
    return true;
}

struct CommitMetadata {
    std::uint32_t messageType;
    std::optional<std::size_t> suffixUnits;
};

// 只有双方协商过的提交才允许解包。保留「无定位」与「显式定位到末尾」的区别。
// text 必须是实际提交正文；继续提交帧先经原载荷解析器去掉消费计数前缀，再校验尾段。
template<class Char>
std::optional<CommitMetadata> Decode(std::uint32_t type, const Char *text,
                                     std::size_t capacity, bool worker, bool negotiated) {
    if (!HasCursor(type)) return CommitMetadata{type, std::nullopt};
    const auto units = SuffixUnits(type, worker);
    if (!negotiated || !units || !ValidText(text, capacity, *units)) return std::nullopt;
    return CommitMetadata{type & 0xffu, units};
}
} // namespace FanyImeQuickPhraseCursorPipe
